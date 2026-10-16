//! Unix domain socket adapter (macOS, Linux, WSL2).

use super::{IpcError, IpcListener};
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener as StdUnixListener, UnixStream as StdUnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Connected server-side stream type.
pub type Stream = tokio::net::UnixStream;

/// Resolve the socket location: `$XDG_RUNTIME_DIR/paneship.sock` when
/// available, otherwise `~/.cache/paneship/paneship.sock`, and finally a
/// per-user temp file as a last resort.
pub fn endpoint_path() -> PathBuf {
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        let dir = PathBuf::from(runtime);
        if !dir.as_os_str().is_empty() {
            return dir.join("paneship.sock");
        }
    }

    if let Some(home) = std::env::var_os("HOME") {
        let dir = PathBuf::from(home);
        if !dir.as_os_str().is_empty() {
            return dir.join(".cache").join("paneship").join("paneship.sock");
        }
    }

    std::env::temp_dir().join(format!("paneship-{}.sock", user_id()))
}

fn user_id() -> u32 {
    // SAFETY: `geteuid` has no preconditions and never fails.
    unsafe { libc::geteuid() }
}

/// Server listener wrapping a tokio [`tokio::net::UnixListener`].
pub struct Listener {
    inner: tokio::net::UnixListener,
}

/// Bind the default endpoint, creating the parent directory when required.
pub fn bind_default() -> io::Result<Listener> {
    bind_at(&endpoint_path())
}

/// Bind a specific socket path (used by tests and custom endpoints).
pub fn bind_at(path: &Path) -> io::Result<Listener> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    let std_listener = StdUnixListener::bind(path)?;
    std_listener.set_nonblocking(true)?;
    let inner = tokio::net::UnixListener::from_std(std_listener)?;
    Ok(Listener { inner })
}

#[async_trait::async_trait]
impl IpcListener for Listener {
    type Stream = Stream;

    async fn accept(&mut self) -> Result<Self::Stream, IpcError> {
        let (stream, _addr) = self.inner.accept().await?;
        Ok(stream)
    }
}

/// Blocking client connection. Kept synchronous so `paneship render` never
/// pays for constructing an async runtime.
pub struct ClientConnection {
    inner: StdUnixStream,
}

impl ClientConnection {
    /// Connect to the default endpoint.
    pub fn connect() -> io::Result<Self> {
        Self::connect_at(&endpoint_path())
    }

    /// Connect to a specific socket path.
    pub fn connect_at(path: &Path) -> io::Result<Self> {
        Ok(Self {
            inner: StdUnixStream::connect(path)?,
        })
    }

    /// Set the read timeout.
    pub fn set_read_timeout(&self, timeout: Duration) -> io::Result<()> {
        self.inner.set_read_timeout(Some(timeout))
    }

    /// Set the write timeout.
    pub fn set_write_timeout(&self, timeout: Duration) -> io::Result<()> {
        self.inner.set_write_timeout(Some(timeout))
    }
}

impl Read for ClientConnection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for ClientConnection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// True when the default endpoint accepts connections.
pub fn endpoint_in_use() -> bool {
    endpoint_in_use_at(&endpoint_path())
}

/// True when a specific socket path accepts connections.
pub fn endpoint_in_use_at(path: &Path) -> bool {
    StdUnixStream::connect(path)
        .map(|stream| {
            let _ = stream.shutdown(std::net::Shutdown::Both);
            true
        })
        .unwrap_or(false)
}

/// Remove a stale socket file when no daemon is listening on it.
pub fn cleanup_stale() {
    cleanup_stale_at(&endpoint_path());
}

/// Remove `path` when it is a socket file with no live listener.
pub fn cleanup_stale_at(path: &Path) {
    if path.exists() && !endpoint_in_use_at(path) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener as StdUnixListener;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn unique_socket_path() -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("paneship-ipc-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir.join(format!("sock-{id}"))
    }

    #[test]
    fn stale_socket_is_detected_and_cleaned() {
        let path = unique_socket_path();
        // Bind then drop: the socket file remains but nothing listens on it.
        {
            let _listener = StdUnixListener::bind(&path).unwrap();
        }
        assert!(path.exists(), "socket file should persist after drop");
        assert!(
            !endpoint_in_use_at(&path),
            "stale socket must not read as live"
        );

        cleanup_stale_at(&path);
        assert!(!path.exists(), "stale socket should be removed");
    }

    #[test]
    fn live_socket_is_left_untouched() {
        let path = unique_socket_path();
        let listener = StdUnixListener::bind(&path).unwrap();
        assert!(
            endpoint_in_use_at(&path),
            "bound socket should read as live"
        );

        cleanup_stale_at(&path);
        assert!(path.exists(), "live socket must not be removed");

        drop(listener);
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn listener_accepts_connection() {
        use tokio::io::AsyncReadExt;

        let path = unique_socket_path();
        let mut listener = bind_at(&path).unwrap();

        let client_path = path.clone();
        let client = std::thread::spawn(move || {
            let mut connection = ClientConnection::connect_at(&client_path).unwrap();
            connection.write_all(b"hi").unwrap();
            connection.flush().unwrap();
        });

        let mut stream = listener.accept().await.unwrap();
        let mut buf = [0u8; 2];
        stream.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hi");

        client.join().unwrap();
        let _ = std::fs::remove_file(&path);
    }
}
