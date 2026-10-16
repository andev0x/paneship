//! Paneship background daemon.
//!
//! Serves prompt renders and cached Git/metadata state over the cross-platform
//! [`crate::ipc`] transport. The event loop is async (tokio); expensive Git
//! introspection runs on dedicated worker threads so the accept path never
//! blocks, and a filesystem watcher invalidates cached state when Git metadata
//! changes.

use crate::cache::{GitLookup, GitSnapshot, LanguageSnapshot, MetadataLookup};
use crate::core::prompt::MetadataState;
use crate::detectors::infra::EnvironmentSnapshot;
use crate::ipc::{self, IpcListener};
use crate::modules::git::GitResolution;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

mod log;
pub(crate) mod telemetry;
mod watcher;

use watcher::GitWatcher;

/// Timeout applied to blocking client round trips.
const CLIENT_TIMEOUT: Duration = Duration::from_millis(15);
/// Budget for the first-render synchronous Git computation before falling back.
const FIRST_RENDER_BUDGET_MS: u64 = 30;

#[derive(Serialize, Deserialize)]
pub enum Request {
    GetGit {
        path: PathBuf,
        last_exit_code: i32,
    },
    Render {
        cwd: PathBuf,
        exit_code: i32,
        width: usize,
        duration_ms: Option<u64>,
    },
    NotifyGit(PathBuf, GitSnapshot),
    InvalidateCwd(PathBuf),
    /// Reload `config.toml` from disk (sent by the TUI after a save).
    ReloadConfig,
    Ping,
    /// Request a live telemetry snapshot (`paneship top`).
    Stats,
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Git(GitResolution),
    /// Structured prompt regions: left, right (`RPROMPT`) and cursor.
    Rendered(crate::core::renderer::PromptParts),
    Pong,
    Ok,
    /// Live daemon diagnostics.
    Stats(crate::daemon::telemetry::TelemetrySnapshot),
}

enum WorkerMessage {
    Git(PathBuf),
    Metadata(PathBuf),
    Environment(PathBuf),
}

/// Distinguishes refresh kinds so one never suppresses another for the same
/// working directory.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum UpdateKind {
    Git,
    Metadata,
    Environment,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct UpdateKey {
    kind: UpdateKind,
    path: PathBuf,
}

impl UpdateKey {
    fn git(path: &Path) -> Self {
        Self {
            kind: UpdateKind::Git,
            path: path.to_path_buf(),
        }
    }

    fn metadata(path: &Path) -> Self {
        Self {
            kind: UpdateKind::Metadata,
            path: path.to_path_buf(),
        }
    }

    fn environment(path: &Path) -> Self {
        Self {
            kind: UpdateKind::Environment,
            path: path.to_path_buf(),
        }
    }
}

type WorkerTx = Arc<Mutex<Sender<WorkerMessage>>>;
type UpdatingSet = Arc<Mutex<HashSet<UpdateKey>>>;

#[derive(Clone)]
struct DaemonState {
    tx: WorkerTx,
    updating: UpdatingSet,
    watcher: GitWatcher,
    known_repos: Arc<Mutex<HashSet<PathBuf>>>,
}

/// Run the daemon to completion (blocks until the listener fails).
pub fn run() -> io::Result<()> {
    std::env::set_var("PANESHIP_DAEMON", "1");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    runtime.block_on(async_main())
}

async fn async_main() -> io::Result<()> {
    if ipc::endpoint_in_use() {
        log::info("another daemon is already listening; exiting");
        return Ok(());
    }
    // Reclaim a socket left behind by a crashed daemon.
    ipc::cleanup_stale_endpoint();

    let (tx, rx) = mpsc::channel::<WorkerMessage>();
    let tx: WorkerTx = Arc::new(Mutex::new(tx));
    let updating: UpdatingSet = Arc::new(Mutex::new(HashSet::new()));

    let watcher = GitWatcher::start(tx.clone(), updating.clone());
    spawn_worker(rx, updating.clone());

    let state = DaemonState {
        tx,
        updating,
        watcher,
        known_repos: Arc::new(Mutex::new(HashSet::new())),
    };

    let mut listener = ipc::bind_default()?;
    log::info(format!("daemon listening on {}", ipc::endpoint_display()));

    loop {
        match listener.accept().await {
            Ok(stream) => {
                let state = state.clone();
                tokio::spawn(async move {
                    let _guard = telemetry::ConnectionGuard::new();
                    if let Err(err) = handle_client(stream, state).await {
                        log::warn(format!("client handler error: {err}"));
                    }
                });
            }
            Err(err) => {
                log::warn(format!("accept error: {err}"));
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }
}

fn spawn_worker(rx: mpsc::Receiver<WorkerMessage>, updating: UpdatingSet) {
    let _ = std::thread::Builder::new()
        .name("paneship-worker".to_string())
        .spawn(move || {
            while let Ok(message) = rx.recv() {
                let started = std::time::Instant::now();
                let key = match message {
                    WorkerMessage::Git(path) => {
                        refresh_git(&path);
                        UpdateKey::git(&path)
                    }
                    WorkerMessage::Metadata(path) => {
                        refresh_metadata(&path);
                        UpdateKey::metadata(&path)
                    }
                    WorkerMessage::Environment(path) => {
                        refresh_environment(&path);
                        UpdateKey::environment(&path)
                    }
                };
                remove_updating(&updating, &key);
                telemetry::record_worker_busy(started.elapsed());
            }
        });
}

fn refresh_git(path: &Path) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::modules::git::compute_git_status_raw(path)
    }));

    match result {
        Ok(Some(snapshot)) => crate::cache::set_git(path, snapshot),
        Ok(None) => {
            log::warn(format!("gix produced no status for {}", path.display()));
            crate::cache::mark_git_pending(path);
        }
        Err(_) => {
            log::error(format!("git worker panicked for {}", path.display()));
            crate::cache::mark_git_pending(path);
        }
    }
    telemetry::record_git_refresh();
}

fn refresh_metadata(path: &Path) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::modules::metadata::compute_metadata_for_daemon(path);
    }));
    if result.is_err() {
        log::error(format!("metadata worker panicked for {}", path.display()));
    }
    telemetry::record_metadata_refresh();
}

fn refresh_environment(path: &Path) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::modules::environment::compute_environment_for_daemon(path);
    }));
    if result.is_err() {
        log::error(format!(
            "environment worker panicked for {}",
            path.display()
        ));
    }
    telemetry::record_environment_refresh();
}

fn remove_updating(updating: &UpdatingSet, key: &UpdateKey) {
    if let Ok(mut set) = updating.lock() {
        set.remove(key);
    }
}

fn enqueue_git_refresh(path: &Path, tx: &WorkerTx, updating: &UpdatingSet) {
    let key = UpdateKey::git(path);
    if let Ok(mut set) = updating.lock() {
        if set.insert(key) {
            if let Ok(tx) = tx.lock() {
                let _ = tx.send(WorkerMessage::Git(path.to_path_buf()));
            }
        }
    }
}

fn enqueue_metadata_refresh(path: &Path, tx: &WorkerTx, updating: &UpdatingSet) {
    let key = UpdateKey::metadata(path);
    if let Ok(mut set) = updating.lock() {
        if set.insert(key) {
            if let Ok(tx) = tx.lock() {
                let _ = tx.send(WorkerMessage::Metadata(path.to_path_buf()));
            }
        }
    }
}

fn enqueue_environment_refresh(path: &Path, tx: &WorkerTx, updating: &UpdatingSet) {
    let key = UpdateKey::environment(path);
    if let Ok(mut set) = updating.lock() {
        if set.insert(key) {
            if let Ok(tx) = tx.lock() {
                let _ = tx.send(WorkerMessage::Environment(path.to_path_buf()));
            }
        }
    }
}

fn register_repo(state: &DaemonState, repo: &Path) {
    if let Ok(mut set) = state.known_repos.lock() {
        if set.insert(repo.to_path_buf()) {
            state.watcher.watch_repo(repo);
        }
    }
}

async fn handle_client(stream: ipc::Stream, state: DaemonState) -> io::Result<()> {
    let mut stream = stream;
    let request: Request = read_async(&mut stream).await?;

    let response = match request {
        Request::GetGit {
            path,
            last_exit_code,
        } => Response::Git(resolve_git_for_render(&path, last_exit_code, &state).await),
        Request::Render {
            cwd,
            exit_code,
            width,
            duration_ms,
        } => {
            telemetry::record_render_request();
            let resolution = resolve_git_for_render(&cwd, exit_code, &state).await;
            let language_state = resolve_language_for_render(&cwd, &state);
            let package_state = resolve_package_for_render(&cwd, &state);
            let environment_state = resolve_environment_for_render(&cwd, &state);
            let mut context = crate::core::prompt::PromptContext::from_inputs(
                Some(cwd),
                Some(width),
                exit_code,
                duration_ms,
            );
            context.git_state = Some(resolution);
            context.language_state = Some(language_state);
            context.package_state = Some(package_state);
            context.environment_state = Some(environment_state);
            Response::Rendered(crate::core::renderer::render_parts(&context))
        }
        Request::NotifyGit(path, snapshot) => {
            crate::cache::set_git(&path, snapshot);
            Response::Ok
        }
        Request::InvalidateCwd(path) => {
            // Clear the repository-root resolution first so a directory that
            // just became (or stopped being) a repository is re-detected — for
            // example immediately after `git init`.
            crate::cache::invalidate_repo_root(&path);
            let repo = crate::cache::repo_root_for(&path).unwrap_or(path);
            crate::cache::invalidate_git(&repo);
            enqueue_git_refresh(&repo, &state.tx, &state.updating);
            register_repo(&state, &repo);
            Response::Ok
        }
        Request::ReloadConfig => {
            let _ = crate::core::config::Config::reload();
            crate::cache::invalidate_metadata();
            log::info("configuration reloaded via IPC");
            Response::Ok
        }
        Request::Ping => Response::Pong,
        Request::Stats => Response::Stats(telemetry::snapshot()),
    };

    write_async(&mut stream, &response).await?;
    Ok(())
}

/// Resolve Git state for a render, triggering background refresh and returning
/// stale data or an explicit pending placeholder rather than nothing.
async fn resolve_git_for_render(
    cwd: &Path,
    _last_exit_code: i32,
    state: &DaemonState,
) -> GitResolution {
    let Some(repo) = crate::cache::repo_root_for(cwd) else {
        return GitResolution::NotRepository;
    };
    register_repo(state, &repo);

    match crate::cache::git_lookup(&repo) {
        GitLookup::Fresh(snapshot) => GitResolution::Ready {
            snapshot,
            stale: false,
        },
        GitLookup::Stale(Some(snapshot)) => {
            enqueue_git_refresh(&repo, &state.tx, &state.updating);
            GitResolution::Ready {
                snapshot,
                stale: true,
            }
        }
        GitLookup::Stale(None) => {
            enqueue_git_refresh(&repo, &state.tx, &state.updating);
            GitResolution::Pending
        }
        GitLookup::Missing => {
            crate::cache::mark_git_pending(&repo);
            enqueue_git_refresh(&repo, &state.tx, &state.updating);

            let repo_for_compute = repo.clone();
            let computed = tokio::task::spawn_blocking(move || {
                crate::modules::git::compute_git_status_with_timeout(
                    &repo_for_compute,
                    FIRST_RENDER_BUDGET_MS,
                )
            })
            .await
            .ok()
            .flatten();

            match computed {
                Some(snapshot) => {
                    crate::cache::set_git(&repo, snapshot.clone());
                    GitResolution::Ready {
                        snapshot,
                        stale: false,
                    }
                }
                None => {
                    log::warn(format!(
                        "git state unavailable within {FIRST_RENDER_BUDGET_MS}ms for {}; serving placeholder",
                        repo.display()
                    ));
                    GitResolution::Pending
                }
            }
        }
    }
}

/// Resolve language metadata for a render without blocking on manifest I/O.
///
/// A cheap marker probe decides whether a language applies at all; the actual
/// version extraction is dispatched to the worker pool, returning cached/stale
/// data or an explicit `[lang...]` placeholder in the meantime.
fn resolve_language_for_render(cwd: &Path, state: &DaemonState) -> MetadataState<LanguageSnapshot> {
    let Some(matched) = crate::detectors::find_marker(cwd) else {
        return MetadataState::Absent;
    };

    match crate::cache::language_lookup(&matched.marker) {
        MetadataLookup::Fresh(value) => MetadataState::Ready {
            value,
            stale: false,
        },
        MetadataLookup::Stale(Some(value)) => {
            enqueue_metadata_refresh(cwd, &state.tx, &state.updating);
            MetadataState::Ready { value, stale: true }
        }
        MetadataLookup::Stale(None) => {
            enqueue_metadata_refresh(cwd, &state.tx, &state.updating);
            MetadataState::Pending
        }
        MetadataLookup::Missing => {
            crate::cache::mark_language_pending(&matched.marker);
            enqueue_metadata_refresh(cwd, &state.tx, &state.updating);
            MetadataState::Pending
        }
    }
}

/// Resolve the package version for a render using the same non-blocking
/// stale-while-revalidate pattern as language metadata.
fn resolve_package_for_render(cwd: &Path, state: &DaemonState) -> MetadataState<String> {
    let Some(manifest) = crate::modules::package::find_cargo_manifest_path(cwd) else {
        return MetadataState::Absent;
    };

    match crate::cache::package_lookup(&manifest) {
        MetadataLookup::Fresh(value) => MetadataState::Ready {
            value,
            stale: false,
        },
        MetadataLookup::Stale(Some(value)) => {
            enqueue_metadata_refresh(cwd, &state.tx, &state.updating);
            MetadataState::Ready { value, stale: true }
        }
        MetadataLookup::Stale(None) => {
            enqueue_metadata_refresh(cwd, &state.tx, &state.updating);
            MetadataState::Pending
        }
        MetadataLookup::Missing => {
            crate::cache::mark_package_pending(&manifest);
            enqueue_metadata_refresh(cwd, &state.tx, &state.updating);
            MetadataState::Pending
        }
    }
}

/// Resolve infrastructure/environment state for a render using the same
/// non-blocking stale-while-revalidate pattern as language metadata.
fn resolve_environment_for_render(
    cwd: &Path,
    state: &DaemonState,
) -> MetadataState<EnvironmentSnapshot> {
    match crate::cache::environment_lookup(cwd) {
        MetadataLookup::Fresh(value) => MetadataState::Ready {
            value,
            stale: false,
        },
        MetadataLookup::Stale(Some(value)) => {
            enqueue_environment_refresh(cwd, &state.tx, &state.updating);
            MetadataState::Ready { value, stale: true }
        }
        MetadataLookup::Stale(None) => {
            enqueue_environment_refresh(cwd, &state.tx, &state.updating);
            MetadataState::Pending
        }
        MetadataLookup::Missing => {
            crate::cache::mark_environment_pending(cwd);
            enqueue_environment_refresh(cwd, &state.tx, &state.updating);
            MetadataState::Pending
        }
    }
}

// ---------------------------------------------------------------------------
// Client API (blocking, no async runtime required)
// ---------------------------------------------------------------------------

fn connect() -> io::Result<ipc::ClientConnection> {
    let mut connection = ipc::ClientConnection::connect()?;
    let _ = connection.set_read_timeout(CLIENT_TIMEOUT);
    let _ = connection.set_write_timeout(CLIENT_TIMEOUT);
    Ok(connection)
}

fn send(request: &Request) -> io::Result<Response> {
    let mut connection = connect()?;
    write_sync(&mut connection, request)?;
    read_sync(&mut connection)
}

/// Query cached/computed Git state for `path`.
pub fn query_git(path: &Path, last_exit_code: i32) -> Option<GitResolution> {
    let request = Request::GetGit {
        path: path.to_path_buf(),
        last_exit_code,
    };
    match send(&request) {
        Ok(Response::Git(resolution)) => Some(resolution),
        _ => None,
    }
}

/// Ask the daemon to reload `config.toml` (sent by the TUI after a save).
pub fn reload_config() -> bool {
    matches!(send(&Request::ReloadConfig), Ok(Response::Ok))
}

/// Ask the daemon to render the prompt regions. Returns `None` when unreachable.
pub fn render(
    cwd: PathBuf,
    exit_code: i32,
    width: usize,
    duration_ms: Option<u64>,
) -> Option<crate::core::renderer::PromptParts> {
    let request = Request::Render {
        cwd,
        exit_code,
        width,
        duration_ms,
    };
    match send(&request) {
        Ok(Response::Rendered(rendered)) => Some(rendered),
        _ => None,
    }
}

/// Signal that Git metadata under `cwd` changed (post-`git` command hook).
pub fn invalidate(cwd: &Path) -> bool {
    let request = Request::InvalidateCwd(cwd.to_path_buf());
    matches!(send(&request), Ok(Response::Ok))
}

/// Ping the daemon.
pub fn ping() -> bool {
    matches!(send(&Request::Ping), Ok(Response::Pong))
}

/// Fetch a live telemetry snapshot for `paneship top`. Returns `None` when the
/// daemon is unreachable.
pub fn query_telemetry() -> Option<telemetry::TelemetrySnapshot> {
    match send(&Request::Stats) {
        Ok(Response::Stats(snapshot)) => Some(snapshot),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Framing (length-prefixed bincode)
// ---------------------------------------------------------------------------

fn encode_message<T: Serialize>(message: &T) -> io::Result<Vec<u8>> {
    let payload = bincode::serialize(message).map_err(to_io)?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn to_io(err: bincode::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, err)
}

fn read_frame(len: usize, buffer: &mut Vec<u8>) -> io::Result<()> {
    if len > ipc::MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ipc frame exceeds maximum size",
        ));
    }
    buffer.resize(len, 0);
    Ok(())
}

fn write_sync<W: Write, T: Serialize>(writer: &mut W, message: &T) -> io::Result<()> {
    let frame = encode_message(message)?;
    writer.write_all(&frame)?;
    writer.flush()
}

fn read_sync<R: Read, T: DeserializeOwned>(reader: &mut R) -> io::Result<T> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buffer = Vec::new();
    read_frame(len, &mut buffer)?;
    reader.read_exact(&mut buffer)?;
    bincode::deserialize(&buffer).map_err(to_io)
}

async fn write_async<W: tokio::io::AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    message: &T,
) -> io::Result<()> {
    let frame = encode_message(message)?;
    writer.write_all(&frame).await?;
    writer.flush().await
}

async fn read_async<R: tokio::io::AsyncRead + Unpin, T: DeserializeOwned>(
    reader: &mut R,
) -> io::Result<T> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buffer = Vec::new();
    read_frame(len, &mut buffer)?;
    reader.read_exact(&mut buffer).await?;
    bincode::deserialize(&buffer).map_err(to_io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framed_request_round_trips() {
        let frame = encode_message(&Request::Ping).unwrap();
        assert_eq!(
            u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize,
            frame.len() - 4
        );

        let mut cursor = std::io::Cursor::new(frame);
        let decoded: Request = read_sync(&mut cursor).unwrap();
        assert!(matches!(decoded, Request::Ping));
    }

    #[test]
    fn framed_response_round_trips() {
        let parts = crate::core::renderer::PromptParts {
            left: "left".to_string(),
            right: "right".to_string(),
            cursor: "> ".to_string(),
        };
        let frame = encode_message(&Response::Rendered(parts)).unwrap();
        let mut cursor = std::io::Cursor::new(frame);
        let decoded: Response = read_sync(&mut cursor).unwrap();
        match decoded {
            Response::Rendered(parts) => {
                assert_eq!(parts.left, "left");
                assert_eq!(parts.right, "right");
                assert_eq!(parts.cursor, "> ");
            }
            _ => panic!("unexpected response"),
        }
    }

    #[test]
    fn oversized_frame_is_rejected() {
        let mut buffer = Vec::new();
        let error = read_frame(ipc::MAX_FRAME + 1, &mut buffer).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
