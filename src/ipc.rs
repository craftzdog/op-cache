//! Local transport between the client and the daemon: a Unix domain socket on
//! Unix and a named pipe on Windows. Both are restricted to the current user.

#[cfg(unix)]
pub use self::unix::*;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(unix)]
mod unix {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use tokio::net::{UnixListener, UnixStream};

    use crate::error::Error;

    pub type ClientStream = UnixStream;
    pub type ServerStream = UnixStream;

    pub struct Listener(UnixListener);

    impl Listener {
        /// Binds `path`, replacing any stale socket and restricting it to the owner.
        pub fn bind(path: &Path) -> Result<Self, Error> {
            let _ = std::fs::remove_file(path);

            // Set restrictive umask before binding to avoid permission race window
            let old_umask = unsafe { libc::umask(0o177) };
            let bind_result = UnixListener::bind(path);
            unsafe { libc::umask(old_umask) };

            let listener = bind_result
                .map_err(|e| Error::Internal(format!("failed to bind socket: {}", e)))?;

            // Verify permissions (belt and suspenders)
            if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
                let _ = std::fs::remove_file(path);
                return Err(Error::Internal(format!(
                    "failed to set socket permissions: {}",
                    e
                )));
            }

            Ok(Self(listener))
        }

        pub async fn accept(&mut self) -> std::io::Result<ServerStream> {
            let (stream, _addr) = self.0.accept().await?;
            Ok(stream)
        }
    }

    pub async fn connect(path: &Path) -> std::io::Result<ClientStream> {
        UnixStream::connect(path).await
    }

    /// Returns whether a daemon is accepting connections on `path`.
    pub fn is_ready(path: &Path) -> bool {
        std::os::unix::net::UnixStream::connect(path).is_ok()
    }

    pub fn remove_endpoint(path: &Path) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::path::Path;
    use std::time::Duration;

    use tokio::net::windows::named_pipe::{
        ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
    };
    use windows_sys::Win32::Foundation::{LocalFree, ERROR_PIPE_BUSY};
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;

    use crate::error::Error;

    pub type ClientStream = NamedPipeClient;
    pub type ServerStream = NamedPipeServer;

    /// The default pipe DACL grants read access to Everyone; a secrets cache must
    /// be as private as the 0600 socket used on Unix.
    const OWNER_ONLY_PIPE_SDDL: &str = "D:P(A;;GA;;;OW)";
    const PIPE_BUSY_RETRY_INTERVAL: Duration = Duration::from_millis(10);
    const PIPE_BUSY_MAX_RETRIES: u32 = 100;

    fn is_pipe_busy(error: &std::io::Error) -> bool {
        error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32)
    }

    fn create_pipe_instance(path: &Path, is_first: bool) -> std::io::Result<NamedPipeServer> {
        let sddl: Vec<u16> = OWNER_ONLY_PIPE_SDDL
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut descriptor = std::ptr::null_mut();
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                std::ptr::null_mut(),
            )
        };
        if converted == 0 {
            return Err(std::io::Error::last_os_error());
        }

        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let created = unsafe {
            ServerOptions::new()
                .first_pipe_instance(is_first)
                .reject_remote_clients(true)
                .create_with_security_attributes_raw(
                    path,
                    &mut attributes as *mut SECURITY_ATTRIBUTES as *mut c_void,
                )
        };
        unsafe { LocalFree(descriptor) };
        created
    }

    pub struct Listener {
        path: std::path::PathBuf,
        next_instance: NamedPipeServer,
    }

    impl Listener {
        /// Creates the pipe, failing if another process already owns the name.
        pub fn bind(path: &Path) -> Result<Self, Error> {
            let next_instance = create_pipe_instance(path, true)
                .map_err(|e| Error::Internal(format!("failed to create pipe: {}", e)))?;
            Ok(Self {
                path: path.to_path_buf(),
                next_instance,
            })
        }

        pub async fn accept(&mut self) -> std::io::Result<ServerStream> {
            self.next_instance.connect().await?;
            // The replacement must exist before the connected instance is handed
            // off, or clients arriving in the gap would see the pipe as missing.
            let replacement = create_pipe_instance(&self.path, false)?;
            Ok(std::mem::replace(&mut self.next_instance, replacement))
        }
    }

    pub async fn connect(path: &Path) -> std::io::Result<ClientStream> {
        let mut retries = 0;
        loop {
            match ClientOptions::new().open(path) {
                Err(error) if is_pipe_busy(&error) && retries < PIPE_BUSY_MAX_RETRIES => {
                    retries += 1;
                    tokio::time::sleep(PIPE_BUSY_RETRY_INTERVAL).await;
                }
                result => return result,
            }
        }
    }

    /// Returns whether a daemon is accepting connections on `path`.
    pub fn is_ready(path: &Path) -> bool {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
        {
            Ok(_) => true,
            Err(error) => is_pipe_busy(&error),
        }
    }

    /// Named pipes vanish with their last handle, so there is nothing to remove.
    pub fn remove_endpoint(_path: &Path) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn unique_endpoint(test_name: &str) -> PathBuf {
        let name = format!("op-cache-test-{}-{}", test_name, std::process::id());
        if cfg!(windows) {
            PathBuf::from(format!(r"\\.\pipe\{}", name))
        } else {
            std::env::temp_dir().join(name)
        }
    }

    #[tokio::test]
    async fn serves_consecutive_connections() {
        let endpoint = unique_endpoint("consecutive");
        let mut listener = Listener::bind(&endpoint).unwrap();

        for expected_byte in [1u8, 2u8] {
            let mut client = connect(&endpoint).await.unwrap();
            let mut server = listener.accept().await.unwrap();

            client.write_all(&[expected_byte]).await.unwrap();
            assert_eq!(server.read_u8().await.unwrap(), expected_byte);
        }

        remove_endpoint(&endpoint);
    }

    #[tokio::test]
    async fn is_ready_tracks_listener_lifetime() {
        let endpoint = unique_endpoint("ready");
        assert!(!is_ready(&endpoint));

        let listener = Listener::bind(&endpoint).unwrap();
        assert!(is_ready(&endpoint));

        drop(listener);
        remove_endpoint(&endpoint);
        assert!(!is_ready(&endpoint));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn rejects_second_daemon_on_same_pipe() {
        let endpoint = unique_endpoint("duplicate");
        let _first = Listener::bind(&endpoint).unwrap();
        assert!(Listener::bind(&endpoint).is_err());
    }
}
