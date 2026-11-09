use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_socket_path")]
    pub socket_path: PathBuf,

    #[serde(default = "default_ttl_seconds")]
    pub ttl_seconds: u64,

    #[serde(default = "default_max_entries")]
    pub max_entries: u64,

    #[serde(default = "default_op_path")]
    pub op_path: String,

    #[serde(default = "default_op_timeout_seconds")]
    pub op_timeout_seconds: u64,
}

#[cfg(unix)]
fn default_socket_path() -> PathBuf {
    PathBuf::from("/tmp/op-cache.sock")
}

/// Pipe names are machine-global, so each user needs their own.
#[cfg(windows)]
fn default_socket_path() -> PathBuf {
    let username = std::env::var("USERNAME").unwrap_or_else(|_| "default".to_string());
    PathBuf::from(format!(r"\\.\pipe\op-cache-{}", username))
}

fn default_ttl_seconds() -> u64 {
    86400 // 24 hours
}

fn default_max_entries() -> u64 {
    1000
}

fn default_op_path() -> String {
    "op".to_string()
}

fn default_op_timeout_seconds() -> u64 {
    30
}

impl Default for Config {
    fn default() -> Self {
        Self {
            socket_path: default_socket_path(),
            ttl_seconds: default_ttl_seconds(),
            max_entries: default_max_entries(),
            op_path: default_op_path(),
            op_timeout_seconds: default_op_timeout_seconds(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            let config: Config = serde_saphyr::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub fn config_path() -> Result<PathBuf> {
        if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "op-cache") {
            Ok(proj_dirs.config_dir().join("config.yaml"))
        } else {
            Ok(PathBuf::from("~/.config/op-cache/config.yaml"))
        }
    }

    pub fn pid_path(&self) -> PathBuf {
        self.daemon_file_path("pid")
    }

    pub fn log_path(&self) -> PathBuf {
        self.daemon_file_path("log")
    }

    #[cfg(unix)]
    fn daemon_file_path(&self, extension: &str) -> PathBuf {
        self.socket_path.with_extension(extension)
    }

    /// A named pipe path is not on the filesystem, so these files go in the
    /// per-user temp directory instead of next to it.
    #[cfg(windows)]
    fn daemon_file_path(&self, extension: &str) -> PathBuf {
        let pipe_name = self
            .socket_path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("op-cache"))
            .to_string_lossy();
        std::env::temp_dir().join(format!("{}.{}", pipe_name, extension))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn daemon_files_live_in_temp_dir_not_pipe_namespace() {
        let config = Config {
            socket_path: PathBuf::from(r"\\.\pipe\op-cache-tester"),
            ..Config::default()
        };

        assert_eq!(
            config.pid_path(),
            std::env::temp_dir().join("op-cache-tester.pid")
        );
        assert_eq!(
            config.log_path(),
            std::env::temp_dir().join("op-cache-tester.log")
        );
    }
}
