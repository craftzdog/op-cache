use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to connect to daemon: {0}")]
    ConnectionFailed(#[from] std::io::Error),

    #[error("failed to spawn daemon: {0}")]
    SpawnFailed(String),

    #[error("daemon failed to start within timeout")]
    DaemonStartTimeout,

    #[error("op command failed: {0}")]
    OpFailed(String),

    #[error("invalid secret reference: {0}")]
    InvalidReference(String),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Error::Internal(e.to_string())
    }
}
