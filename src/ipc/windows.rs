//! Windows named pipe adapter.
//!
//! Named pipes give native, low-latency local IPC parity with the Unix domain
//! socket transport. Production hardening (Phase 3, Module C):
//!
//! * every pipe instance is created with an explicit DACL granting access to
//!   the **current user only** (with a safe fallback to the process default
//!   DACL if the descriptor cannot be built);
//! * remote clients are rejected (`PIPE_REJECT_REMOTE_CLIENTS`);
//! * the blocking client retries `ERROR_PIPE_BUSY` with exponential backoff,
//!   which is the documented "all instances busy" transient;
//! * the read timeout requested by the client is enforced with `PeekNamedPipe`
//!   polling instead of being silently ignored.
//!
//! Each accepted connection requires a fresh pipe instance, which
//! [`Listener::accept`] takes care of transparently.

use super::{IpcError, IpcListener};
use std::io::{self, Read, Write};
use std::os::windows::io::AsRawHandle;
use std::time::{Duration, Instant};
use tokio::net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions};

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, HANDLE};
use windows_sys::Win32::Security::{
    AddAccessAllowedAce, GetLengthSid, GetTokenInformation, InitializeAcl,
    InitializeSecurityDescriptor, SetSecurityDescriptorDacl, TokenUser, ACCESS_ALLOWED_ACE, ACL,
    ACL_REVISION, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR, TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::System::Pipes::PeekNamedPipe;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Connected server-side stream type.
pub type Stream = NamedPipeServer;

/// `SECURITY_DESCRIPTOR_REVISION` (Win32 SDK).
const SECURITY_DESCRIPTOR_REVISION: u32 = 1;
/// `GENERIC_READ | GENERIC_WRITE`, granted to the current user.
const PIPE_ACCESS: u32 = 0x8000_0000 | 0x4000_0000;
/// Total connection attempts before surfacing `ERROR_PIPE_BUSY`.
const CONNECT_ATTEMPTS: u32 = 6;
/// Base delay for the exponential `ERROR_PIPE_BUSY` backoff.
const CONNECT_BACKOFF: Duration = Duration::from_millis(2);

/// Resolve the named pipe path for the current user.
///
/// The user name is sanitised so an unusual `USERNAME` cannot break out of the
/// pipe namespace.
pub fn endpoint_name() -> String {
    let raw = std::env::var("USERNAME").unwrap_or_else(|_| "default".to_string());
    let user: String = raw
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect();
    let user = if user.is_empty() {
        "default".to_string()
    } else {
        user
    };
    format!(r"\\.\pipe\paneship-{user}")
}

/// Heap-backed security descriptor + ACL kept alive while the pipe is created.
struct PipeSecurity {
    attrs: SECURITY_ATTRIBUTES,
    _descriptor: Box<SECURITY_DESCRIPTOR>,
    _acl: Vec<u8>,
}

/// Build `SECURITY_ATTRIBUTES` granting the current user read/write access.
///
/// Returns `None` when the token/ACL cannot be built; callers then fall back to
/// the process default DACL (still logon-session scoped).
fn build_security_attributes() -> Option<PipeSecurity> {
    // SAFETY: the token handle is closed before returning; all buffers passed to
    // Win32 outlive their respective calls and are kept alive by `PipeSecurity`.
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return None;
        }
        let result = build_inner(token);
        let _ = CloseHandle(token);
        result
    }
}

unsafe fn build_inner(token: HANDLE) -> Option<PipeSecurity> {
    // 1. Resolve the current user's SID.
    let mut needed = 0u32;
    GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
    if needed == 0 {
        return None;
    }

    let mut token_buffer = vec![0u8; needed as usize];
    if GetTokenInformation(
        token,
        TokenUser,
        token_buffer.as_mut_ptr() as *mut core::ffi::c_void,
        needed,
        &mut needed,
    ) == 0
    {
        return None;
    }

    let token_user = &*(token_buffer.as_ptr() as *const TOKEN_USER);
    let sid = token_user.User.Sid;
    let sid_len = GetLengthSid(sid) as usize;
    if sid_len == 0 {
        return None;
    }

    // 2. DACL sized for a single ACCESS_ALLOWED_ACE plus the SID.
    let ace_len = std::mem::size_of::<ACCESS_ALLOWED_ACE>() - std::mem::size_of::<u32>() + sid_len;
    let acl_len = align4(std::mem::size_of::<ACL>() + ace_len);
    let mut acl = vec![0u8; acl_len];
    let acl_ptr = acl.as_mut_ptr() as *mut ACL;
    if InitializeAcl(acl_ptr, acl_len as u32, ACL_REVISION) == 0 {
        return None;
    }
    if AddAccessAllowedAce(acl_ptr, ACL_REVISION, PIPE_ACCESS, sid) == 0 {
        return None;
    }

    // 3. Security descriptor carrying that DACL (never "defaulted").
    let mut descriptor: Box<SECURITY_DESCRIPTOR> = Box::new(std::mem::zeroed());
    let descriptor_ptr = descriptor.as_mut() as *mut SECURITY_DESCRIPTOR;
    let raw_descriptor = descriptor_ptr as *mut core::ffi::c_void;
    if InitializeSecurityDescriptor(raw_descriptor, SECURITY_DESCRIPTOR_REVISION) == 0 {
        return None;
    }
    if SetSecurityDescriptorDacl(raw_descriptor, 1, acl_ptr as *const ACL, 0) == 0 {
        return None;
    }

    let attrs = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: raw_descriptor,
        bInheritHandle: 0,
    };

    Some(PipeSecurity {
        attrs,
        _descriptor: descriptor,
        _acl: acl,
    })
}

fn align4(value: usize) -> usize {
    (value + 3) & !3
}

fn new_instance(name: &str, first: bool) -> io::Result<NamedPipeServer> {
    let mut options = ServerOptions::new();
    options.pipe_mode(PipeMode::Byte);
    options.reject_remote_clients(true);
    if first {
        options.first_pipe_instance(true);
    }

    // Security buffers are rebuilt per instance and dropped immediately after
    // creation: the kernel copies the descriptor, so `Listener` stays `Send`.
    match build_security_attributes() {
        Some(security) => {
            let attrs = &security.attrs as *const SECURITY_ATTRIBUTES as *mut core::ffi::c_void;
            // SAFETY: `attrs` points at a live `SECURITY_ATTRIBUTES` whose
            // descriptor/ACL buffers outlive the call.
            unsafe { options.create_with_security_attributes_raw(name, attrs) }
        }
        None => options.create(name),
    }
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
    read_timeout: Option<Duration>,
}

impl ClientConnection {
    /// Connect to the default pipe.
    pub fn connect() -> io::Result<Self> {
        Self::connect_at(&endpoint_name())
    }

    /// Connect to a specific pipe name, retrying `ERROR_PIPE_BUSY` with
    /// exponential backoff while all server instances are momentarily busy.
    pub fn connect_at(name: &str) -> io::Result<Self> {
        let mut attempt = 0u32;
        loop {
            match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(name)
            {
                Ok(inner) => {
                    return Ok(Self {
                        inner,
                        read_timeout: None,
                    })
                }
                Err(err) => {
                    let busy = err.raw_os_error() == Some(ERROR_PIPE_BUSY as i32);
                    if busy && attempt + 1 < CONNECT_ATTEMPTS {
                        std::thread::sleep(CONNECT_BACKOFF * (1u32 << attempt));
                        attempt += 1;
                        continue;
                    }
                    return Err(err);
                }
            }
        }
    }

    /// Store a read timeout enforced during [`Read::read`].
    pub fn set_read_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.read_timeout = Some(timeout);
        Ok(())
    }

    /// Windows pipe writes are not interruptible; accepted but ignored.
    pub fn set_write_timeout(&self, _timeout: Duration) -> io::Result<()> {
        Ok(())
    }

    /// Poll until the pipe has data available, a timeout elapses, or the pipe
    /// is broken. A broken pipe returns `Ok` so the subsequent `read` reports
    /// the precise error.
    fn wait_readable(&self, timeout: Duration) -> io::Result<()> {
        let handle = self.inner.as_raw_handle() as HANDLE;
        let deadline = Instant::now() + timeout;

        loop {
            let mut available = 0u32;
            // SAFETY: a null peek buffer with zero size only requests the
            // available byte count; all out-pointers are valid.
            let ok = unsafe {
                PeekNamedPipe(
                    handle,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 || available > 0 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "named pipe read timed out",
                ));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

impl Read for ClientConnection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(timeout) = self.read_timeout {
            self.wait_readable(timeout)?;
        }
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
///
/// `ERROR_PIPE_BUSY` is treated as "in use" (the retry loop resolves it);
/// a missing pipe (`ERROR_FILE_NOT_FOUND`) means no daemon is listening.
pub fn endpoint_in_use() -> bool {
    match ClientConnection::connect() {
        Ok(connection) => {
            drop(connection);
            true
        }
        Err(err) => err.raw_os_error() != Some(ERROR_FILE_NOT_FOUND as i32),
    }
}

/// Named pipes have no on-disk artifact to clean up.
pub fn cleanup_stale() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_name_is_namespaced_and_sanitised() {
        let name = endpoint_name();
        assert!(name.starts_with(r"\\.\pipe\paneship-"));
        let suffix = name.trim_start_matches(r"\\.\pipe\paneship-");
        assert!(
            suffix
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'),
            "unexpected characters in pipe name: {name}"
        );
    }

    #[test]
    fn acl_size_alignment_is_rounding_up() {
        assert_eq!(align4(1), 4);
        assert_eq!(align4(4), 4);
        assert_eq!(align4(5), 8);
    }
}
