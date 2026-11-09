use std::path::Path;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::daemon;
use crate::error::Error;
use crate::ipc;

const SPAWN_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Spawn the daemon if not already running
pub fn ensure_daemon_running(config: &Config) -> Result<(), Error> {
    if daemon::is_running(config) {
        return Ok(());
    }

    spawn_daemon()?;
    wait_for_daemon(config)
}

fn spawn_daemon() -> Result<(), Error> {
    let exe_path = std::env::current_exe()
        .map_err(|e| Error::SpawnFailed(format!("failed to get executable path: {}", e)))?;

    start_daemon_process(&exe_path)
        .map_err(|e| Error::SpawnFailed(format!("failed to spawn daemon: {}", e)))
}

#[cfg(unix)]
fn start_daemon_process(exe_path: &Path) -> std::io::Result<()> {
    // Spawn the daemon process - it will fork and daemonize itself
    std::process::Command::new(exe_path)
        .arg("daemon")
        .spawn()
        .map(drop)
}

/// Starts the daemon with no console, no inherited handles, and a working
/// directory that doesn't pin the caller's.
///
/// `std::process::Command` always enables handle inheritance, which would leak
/// any pipe a calling shell has open to the long-lived daemon. Whoever reads
/// that pipe, as in `$secret = op-cache read ...`, would then never see EOF.
#[cfg(windows)]
fn start_daemon_process(exe_path: &Path) -> std::io::Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        CreateProcessW, CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS, PROCESS_INFORMATION,
        STARTUPINFOW,
    };

    fn to_wide_nul(text: &OsStr) -> Vec<u16> {
        text.encode_wide().chain(std::iter::once(0)).collect()
    }

    let application = to_wide_nul(exe_path.as_os_str());
    let mut command_line = to_wide_nul(OsStr::new("op-cache daemon"));
    let working_directory = to_wide_nul(std::env::temp_dir().as_os_str());
    let startup_info = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut process_info = PROCESS_INFORMATION::default();

    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            command_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,
            std::ptr::null(),
            working_directory.as_ptr(),
            &startup_info,
            &mut process_info,
        )
    };
    if created == 0 {
        return Err(std::io::Error::last_os_error());
    }

    unsafe {
        CloseHandle(process_info.hProcess);
        CloseHandle(process_info.hThread);
    }
    Ok(())
}

fn wait_for_daemon(config: &Config) -> Result<(), Error> {
    let start = Instant::now();

    while start.elapsed() < SPAWN_TIMEOUT {
        if ipc::is_ready(&config.socket_path) {
            return Ok(());
        }
        std::thread::sleep(POLL_INTERVAL);
    }

    Err(Error::DaemonStartTimeout)
}
