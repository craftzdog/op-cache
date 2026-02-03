use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "op-cache")]
#[command(about = "A fast caching proxy for 1Password CLI op read commands")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Read a secret (cached)
    Read {
        /// Secret reference (e.g., op://vault/item/field)
        reference: String,
    },

    /// Check if the daemon is running
    Status,

    /// Show cache statistics
    Stats,

    /// Clear the cache
    Clear,

    /// Stop the daemon
    Stop,

    /// Run the daemon in background
    #[command(hide = true)]
    Daemon,

    /// Run the daemon in foreground (for debugging)
    #[command(hide = true)]
    DaemonForeground,
}
