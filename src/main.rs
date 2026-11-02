use anyhow::Result;
use battery_notification_util::{config::Config, listener};
use clap::Parser;
use tokio::signal::unix::{SignalKind, signal};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::parse();
    config.validate()?;
    let mut terminate = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    let shutdown = async {
        tokio::select! {
            _ = terminate.recv() => {},
            _ = interrupt.recv() => {},
        }
        eprintln!("listener shutting down");
    };
    let connection = zbus::Connection::system().await?;
    listener::run(&connection, &config, shutdown).await
}
