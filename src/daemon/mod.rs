use crate::cache::{GitSnapshot, LanguageSnapshot};
use serde::{Deserialize, Serialize};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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
    Ping,
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Git(Option<GitSnapshot>),
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

pub fn get_socket_path() -> PathBuf {
    let uid = unsafe { libc::geteuid() };
    std::env::temp_dir().join(format!("paneship-{}.sock", uid))
}

pub fn run() -> std::io::Result<()> {
    std::env::set_var("PANESHIP_DAEMON", "1");
    let socket_path = get_socket_path();
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    let (tx, rx) = mpsc::channel::<WorkerMessage>();
    let tx = Arc::new(Mutex::new(tx));

    // Background worker thread
    thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            match msg {
                WorkerMessage::UpdateGit(path) => {
                    if let Some(snapshot) = crate::modules::git::compute_git_status(&path) {
                        crate::cache::get_or_compute_git(&path, || Some(snapshot));
                    }
                }
                WorkerMessage::UpdateMetadata(path) => {
                    let _ = crate::modules::metadata::compute_metadata_for_daemon(&path);
                }
            }
        }
    });

    let listener = UnixListener::bind(&socket_path)?;
    println!("Daemon listening on {:?}", socket_path);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let _ = handle_client(&mut stream, &tx);
            }
            Err(err) => {
                eprintln!("Error accepting connection: {}", err);
            }
        }
    }
    Ok(())
}

fn handle_client(
    stream: &mut UnixStream,
    tx: &Arc<Mutex<Sender<WorkerMessage>>>,
) -> bincode::Result<()> {
    let request: Request = bincode::deserialize_from(&mut *stream)?;
    let response = match request {
        Request::GetGit {
            path,
            last_exit_code,
        } => {
            let snapshot = crate::cache::get_or_compute_git(&path, || None);
            let needs_refresh = if let Some(ref s) = snapshot {
                // Heuristic: refresh if exit code is non-zero (something might have changed)
                // or if HEAD has changed.
                if last_exit_code != 0 {
                    true
                } else {
                    let current_head = crate::modules::git::get_head_id(&path);
                    current_head.map(|h| h != s.head_id).unwrap_or(true)
                }
            } else {
                true
            };

            if needs_refresh {
                if let Ok(tx) = tx.lock() {
                    let _ = tx.send(WorkerMessage::UpdateGit(path));
                }
            }
            Response::Git(snapshot)
        }
        Request::GetMetadata { path } => {
            let language = crate::cache::get_language(&path).flatten();
            let package = crate::cache::get_package_version(&path).flatten();

            if language.is_none() && package.is_none() {
                if let Ok(tx) = tx.lock() {
                    let _ = tx.send(WorkerMessage::UpdateMetadata(path));
                }
            }

            Response::Metadata { language, package }
        }
        Request::Render {
            cwd,
            exit_code,
            width,
            duration_ms,
        } => {
            let context = crate::core::prompt::PromptContext::from_inputs(
                Some(cwd),
                Some(width),
                exit_code,
                duration_ms,
            );
            let rendered = crate::core::renderer::render(&context);
            Response::Rendered(rendered)
        }
        Request::NotifyGit(path, snapshot) => {
            crate::cache::get_or_compute_git(&path, || Some(snapshot));
            Response::Ok
        }
        Request::Ping => Response::Pong,
    };
    bincode::serialize_into(stream, &response)?;
    Ok(())
}

pub fn query_git(path: &Path, last_exit_code: i32) -> Option<GitSnapshot> {
    let socket_path = get_socket_path();
    let mut stream = UnixStream::connect(socket_path).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(15)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(15)))
        .ok()?;

    let request = Request::GetGit {
        path: path.to_path_buf(),
        last_exit_code,
    };
    bincode::serialize_into(&mut stream, &request).ok()?;

    let response: Response = bincode::deserialize_from(&mut stream).ok()?;
    match response {
        Response::Git(snapshot) => snapshot,
        _ => None,
    }
}

pub fn query_metadata(path: &Path) -> (Option<LanguageSnapshot>, Option<String>) {
    let socket_path = get_socket_path();
    let Ok(mut stream) = UnixStream::connect(socket_path) else {
        return (None, None);
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(10)));

    let request = Request::GetMetadata {
        path: path.to_path_buf(),
    };
    if bincode::serialize_into(&mut stream, &request).is_err() {
        return (None, None);
    }

    let response: Response = match bincode::deserialize_from(&mut stream) {
        Ok(r) => r,
        Err(_) => return (None, None),
    };

    match response {
        Response::Metadata { language, package } => (language, package),
        _ => (None, None),
    }
}

pub fn render(
    cwd: PathBuf,
    exit_code: i32,
    width: usize,
    duration_ms: Option<u64>,
) -> Option<String> {
    let socket_path = get_socket_path();
    let mut stream = UnixStream::connect(socket_path).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(15)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(15)));

    let request = Request::Render {
        cwd,
        exit_code,
        width,
        duration_ms,
    };
    bincode::serialize_into(&mut stream, &request).ok()?;

    let response: Response = bincode::deserialize_from(&mut stream).ok()?;
    match response {
        Response::Rendered(s) => Some(s),
        _ => None,
    }
}

pub fn notify_git(path: &Path, snapshot: GitSnapshot) {
    let socket_path = get_socket_path();
    if let Ok(mut stream) = UnixStream::connect(socket_path) {
        let _ = stream.set_write_timeout(Some(Duration::from_millis(10)));
        let request = Request::NotifyGit(path.to_path_buf(), snapshot);
        let _ = bincode::serialize_into(&mut stream, &request);
    }
}

pub fn ping() -> bool {
    let socket_path = get_socket_path();
    let Ok(mut stream) = UnixStream::connect(socket_path) else {
        return false;
    };

    let _ = stream.set_read_timeout(Some(Duration::from_millis(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(10)));

    if bincode::serialize_into(&mut stream, &Request::Ping).is_err() {
        return false;
    }

    matches!(
        bincode::deserialize_from::<_, Response>(&mut stream),
        Ok(Response::Pong)
    )
}
