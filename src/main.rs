mod cli;
mod client;
mod config;
mod daemon;
mod error;
mod protocol;
mod spawn;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};
use client::Client;
use config::Config;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
        Command::Read { reference } => cmd_read(config, &reference),
        Command::Status => cmd_status(&config),
        Command::Stats => cmd_stats(config),
        Command::Clear => cmd_clear(config),
        Command::Stop => cmd_stop(config),
        Command::Daemon => cmd_daemon(config),
        Command::DaemonForeground => cmd_daemon_foreground(config),
    }
}

fn cmd_read(config: Config, reference: &str) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let client = Client::new(config);
        let value = client.read(reference).await?;
        print!("{}", value);
        Ok(())
    })
}

fn cmd_status(config: &Config) -> Result<()> {
    if daemon::is_running(config) {
        if let Some(pid) = daemon::get_pid(config) {
            println!("daemon running (pid {})", pid);
        } else {
            println!("daemon running");
        }
    } else {
        println!("daemon not running");
    }
    Ok(())
}

fn cmd_stats(config: Config) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let client = Client::new(config);
        let stats = client.stats().await?;

        println!("Cache Statistics:");
        println!("  Entries: {}", stats.entries);
        println!("  Hits:    {}", stats.hits);
        println!("  Misses:  {}", stats.misses);
        println!("  Hit Rate: {:.1}%", stats.hit_rate * 100.0);

        Ok(())
    })
}

fn cmd_clear(config: Config) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let client = Client::new(config);
        client.clear().await?;
        println!("cache cleared");
        Ok(())
    })
}

fn cmd_stop(config: Config) -> Result<()> {
    if !daemon::is_running(&config) {
        println!("daemon not running");
        return Ok(());
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let client = Client::new(config);
        match client.stop().await {
            Ok(()) => println!("daemon stopped"),
            Err(e) => {
                if e.to_string().contains("Connection reset") {
                    println!("daemon stopped");
                } else {
                    return Err(e.into());
                }
            }
        }
        Ok(())
    })
}

fn cmd_daemon(config: Config) -> Result<()> {
    daemon::daemonize(config)?;
    Ok(())
}

fn cmd_daemon_foreground(config: Config) -> Result<()> {
    daemon::run_foreground(config)?;
    Ok(())
}
