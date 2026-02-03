use std::process::Command;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::daemon;
use crate::error::Error;

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

    // Spawn the daemon process - it will fork and daemonize itself
    Command::new(exe_path)
        .arg("daemon")
        .spawn()
        .map_err(|e| Error::SpawnFailed(format!("failed to spawn daemon: {}", e)))?;

    Ok(())
}

fn wait_for_daemon(config: &Config) -> Result<(), Error> {
    let start = Instant::now();

    while start.elapsed() < SPAWN_TIMEOUT {
        if config.socket_path.exists() {
            if std::os::unix::net::UnixStream::connect(&config.socket_path).is_ok() {
                return Ok(());
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }

    Err(Error::DaemonStartTimeout)
}
