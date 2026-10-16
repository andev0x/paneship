//! Cross-platform IPC transport abstraction.
//!
//! The daemon speaks a length-prefixed bincode protocol over a platform-native
//! local socket: a Unix domain socket on macOS/Linux/WSL2 and a Windows named
//! pipe on native Windows. This module hides that difference behind the
//! [`IpcListener`] trait plus a blocking [`ClientConnection`] used by the
//! latency-sensitive CLI render path (which must never spin up an async runtime).

use std::io;

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

/// Maximum accepted frame size (4 MiB), guarding against runaway allocations.
pub const MAX_FRAME: usize = 4 * 1024 * 1024;

#[cfg(unix)]
pub use unix::{bind_default, Stream};
#[cfg(windows)]
pub use windows::{bind_default, Stream};

pub use platform::ClientConnection;

#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

/// Errors surfaced by the transport layer.
#[derive(Debug)]
pub enum IpcError {
    /// Underlying platform I/O failure.
    Io(io::Error),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpcError::Io(err) => write!(f, "ipc io error: {err}"),
        }
    }
}

impl std::error::Error for IpcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IpcError::Io(err) => Some(err),
        }
    }
}

impl From<io::Error> for IpcError {
    fn from(err: io::Error) -> Self {
        IpcError::Io(err)
    }
}

/// Server-side listener abstraction implemented by every platform adapter.
#[async_trait::async_trait]
pub trait IpcListener: Send {
    /// Connected stream produced by [`IpcListener::accept`].
    type Stream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static;

    /// Wait for the next client connection.
    async fn accept(&mut self) -> Result<Self::Stream, IpcError>;
}

/// Human-readable endpoint description, used for logs and diagnostics.
pub fn endpoint_display() -> String {
    #[cfg(unix)]
    {
        unix::endpoint_path().display().to_string()
    }
    #[cfg(windows)]
    {
        windows::endpoint_name()
    }
}

/// True when a daemon is currently listening on the default endpoint.
///
/// On Unix a stale socket file (left behind by a crashed daemon) refuses the
/// connection, so this returns `false` and lets the caller reclaim the path.
pub fn endpoint_in_use() -> bool {
    platform::endpoint_in_use()
}

/// Remove a stale endpoint artifact (Unix socket file). No-op on Windows,
/// where named pipes disappear with the owning process.
pub fn cleanup_stale_endpoint() {
    platform::cleanup_stale();
}
