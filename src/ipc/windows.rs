//! Windows named pipe adapter.
//!
//! Named pipes give native, low-latency local IPC parity with the Unix domain
//! socket transport. Each accepted connection requires a fresh pipe instance,
//! which [`Listener::accept`] takes care of transparently.

use super::{IpcError, IpcListener};
use std::io::{self, Read, Write};
use std::time::Duration;
use tokio::net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions};

/// Connected server-side stream type.
pub type Stream = NamedPipeServer;

/// Resolve the named pipe path for the current user.
pub fn endpoint_name() -> String {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "default".to_string());
    format!(r"\\.\pipe\paneship-{user}")
}

fn new_instance(name: &str, first: bool) -> io::Result<NamedPipeServer> {
    let mut options = ServerOptions::new();
    options.pipe_mode(PipeMode::Byte);
    if first {
        options.first_pipe_instance(true);
    }
    options.create(name)
}

/// Server listener creating one named pipe instance per connection.
pub struct Listener {
    pipe_name: String,
    server: NamedPipeServer,
}

/// Bind the default pipe.
pub fn bind_default() -> io::Result<Listener> {
    bind_at(&endpoint_name())
}

/// Bind a specific pipe name (used by tests).
pub fn bind_at(name: &str) -> io::Result<Listener> {
    let server = new_instance(name, true)?;
    Ok(Listener {
        pipe_name: name.to_string(),
        server,
    })
}

#[async_trait::async_trait]
impl IpcListener for Listener {
    type Stream = Stream;

    async fn accept(&mut self) -> Result<Self::Stream, IpcError> {
        self.server.connect().await?;
        let next = new_instance(&self.pipe_name, false)?;
        let connected = std::mem::replace(&mut self.server, next);
        Ok(connected)
    }
}

/// Blocking client connection over a named pipe.
pub struct ClientConnection {
    inner: std::fs::File,
}

impl ClientConnection {
    /// Connect to the default pipe.
    pub fn connect() -> io::Result<Self> {
        Self::connect_at(&endpoint_name())
    }

    /// Connect to a specific pipe name.
    pub fn connect_at(name: &str) -> io::Result<Self> {
        let inner = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(name)?;
        Ok(Self { inner })
    }

    /// Windows file handles do not expose read timeouts; accept but ignore.
    pub fn set_read_timeout(&self, _timeout: Duration) -> io::Result<()> {
        Ok(())
    }

    /// Windows file handles do not expose write timeouts; accept but ignore.
    pub fn set_write_timeout(&self, _timeout: Duration) -> io::Result<()> {
        Ok(())
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

/// True when a server responds on the default pipe.
pub fn endpoint_in_use() -> bool {
    ClientConnection::connect().is_ok()
}

/// Named pipes have no on-disk artifact to clean up.
pub fn cleanup_stale() {}
