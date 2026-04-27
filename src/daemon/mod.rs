use crate::cache::GitSnapshot;
use serde::{Deserialize, Serialize};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Serialize, Deserialize)]
pub enum Request {
    GetGit(PathBuf),
    NotifyGit(PathBuf, GitSnapshot),
    Ping,
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Git(Option<GitSnapshot>),
    Pong,
    Ok,
}

pub fn get_socket_path() -> PathBuf {
    let uid = unsafe { libc::geteuid() };
    std::env::temp_dir().join(format!("paneship-{}.sock", uid))
}

pub fn run() -> std::io::Result<()> {
    let socket_path = get_socket_path();
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    println!("Daemon listening on {:?}", socket_path);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let _ = handle_client(&mut stream);
            }
            Err(err) => {
                eprintln!("Error accepting connection: {}", err);
            }
        }
    }
    Ok(())
}

fn handle_client(stream: &mut UnixStream) -> bincode::Result<()> {
    let request: Request = bincode::deserialize_from(&mut *stream)?;
    let response = match request {
        Request::GetGit(path) => {
            let snapshot = crate::cache::get_or_compute_git(&path, || {
                crate::modules::git::compute_git_status(&path)
            });
            Response::Git(snapshot)
        }
        Request::NotifyGit(path, snapshot) => {
            // Update daemon's cache with what the client found
            crate::cache::get_or_compute_git(&path, || Some(snapshot));
            Response::Ok
        }
        Request::Ping => Response::Pong,
    };
    bincode::serialize_into(stream, &response)?;
    Ok(())
}

pub fn query_git(path: &Path) -> Option<GitSnapshot> {
    let socket_path = get_socket_path();
    let mut stream = UnixStream::connect(socket_path).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(10)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(10)))
        .ok()?;

    let request = Request::GetGit(path.to_path_buf());
    bincode::serialize_into(&mut stream, &request).ok()?;

    let response: Response = bincode::deserialize_from(&mut stream).ok()?;
    match response {
        Response::Git(snapshot) => snapshot,
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
