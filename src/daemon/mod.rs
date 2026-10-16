//! Paneship background daemon.
//!
//! Serves prompt renders and cached Git/metadata state over the cross-platform
//! [`crate::ipc`] transport. The event loop is async (tokio); expensive Git
//! introspection runs on dedicated worker threads so the accept path never
//! blocks, and a filesystem watcher invalidates cached state when Git metadata
//! changes.

use crate::cache::{GitLookup, GitSnapshot, LanguageSnapshot};
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
    GetMetadata {
        path: PathBuf,
    },
    Render {
        cwd: PathBuf,
        exit_code: i32,
        width: usize,
        duration_ms: Option<u64>,
    },
    NotifyGit(PathBuf, GitSnapshot),
    InvalidateCwd(PathBuf),
    Ping,
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Git(GitResolution),
    Metadata {
        language: Option<LanguageSnapshot>,
        package: Option<String>,
    },
    Rendered(String),
    Pong,
    Ok,
}

enum WorkerMessage {
    UpdateGit(PathBuf),
    UpdateMetadata(PathBuf),
}

type WorkerTx = Arc<Mutex<Sender<WorkerMessage>>>;
type UpdatingSet = Arc<Mutex<HashSet<PathBuf>>>;

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
                let path = match message {
                    WorkerMessage::UpdateGit(path) => {
                        refresh_git(&path);
                        path
                    }
                    WorkerMessage::UpdateMetadata(path) => {
                        refresh_metadata(&path);
                        path
                    }
                };
                remove_updating(&updating, &path);
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
}

fn refresh_metadata(path: &Path) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::modules::metadata::compute_metadata_for_daemon(path);
    }));
    if result.is_err() {
        log::error(format!("metadata worker panicked for {}", path.display()));
    }
}

fn remove_updating(updating: &UpdatingSet, path: &Path) {
    if let Ok(mut set) = updating.lock() {
        set.remove(path);
    }
}

fn enqueue_git_refresh(path: &Path, tx: &WorkerTx, updating: &UpdatingSet) {
    if let Ok(mut set) = updating.lock() {
        if set.insert(path.to_path_buf()) {
            if let Ok(tx) = tx.lock() {
                let _ = tx.send(WorkerMessage::UpdateGit(path.to_path_buf()));
            }
        }
    }
}

fn enqueue_metadata_refresh(path: &Path, tx: &WorkerTx, updating: &UpdatingSet) {
    if let Ok(mut set) = updating.lock() {
        if set.insert(path.to_path_buf()) {
            if let Ok(tx) = tx.lock() {
                let _ = tx.send(WorkerMessage::UpdateMetadata(path.to_path_buf()));
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
        Request::GetMetadata { path } => {
            let language = crate::cache::get_language(&path).flatten();
            let package = crate::cache::get_package_version(&path).flatten();

            if language.is_none() && package.is_none() {
                enqueue_metadata_refresh(&path, &state.tx, &state.updating);
            }

            Response::Metadata { language, package }
        }
        Request::Render {
            cwd,
            exit_code,
            width,
            duration_ms,
        } => {
            let resolution = resolve_git_for_render(&cwd, exit_code, &state).await;
            let mut context = crate::core::prompt::PromptContext::from_inputs(
                Some(cwd),
                Some(width),
                exit_code,
                duration_ms,
            );
            context.git_state = Some(resolution);
            Response::Rendered(crate::core::renderer::render(&context))
        }
        Request::NotifyGit(path, snapshot) => {
            crate::cache::set_git(&path, snapshot);
            Response::Ok
        }
        Request::InvalidateCwd(path) => {
            let repo = crate::cache::repo_root_for(&path).unwrap_or(path);
            crate::cache::invalidate_git(&repo);
            enqueue_git_refresh(&repo, &state.tx, &state.updating);
            register_repo(&state, &repo);
            Response::Ok
        }
        Request::Ping => Response::Pong,
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

// ---------------------------------------------------------------------------
// Client API (blocking, no async runtime required)
// ---------------------------------------------------------------------------

fn connect() -> io::Result<ipc::ClientConnection> {
    let connection = ipc::ClientConnection::connect()?;
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

/// Query cached language/package metadata for `path`.
pub fn query_metadata(path: &Path) -> (Option<LanguageSnapshot>, Option<String>) {
    let request = Request::GetMetadata {
        path: path.to_path_buf(),
    };
    match send(&request) {
        Ok(Response::Metadata { language, package }) => (language, package),
        _ => (None, None),
    }
}

/// Ask the daemon to render the prompt. Returns `None` when unreachable.
pub fn render(
    cwd: PathBuf,
    exit_code: i32,
    width: usize,
    duration_ms: Option<u64>,
) -> Option<String> {
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
        let frame = encode_message(&Response::Rendered("hello".to_string())).unwrap();
        let mut cursor = std::io::Cursor::new(frame);
        let decoded: Response = read_sync(&mut cursor).unwrap();
        match decoded {
            Response::Rendered(text) => assert_eq!(text, "hello"),
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
