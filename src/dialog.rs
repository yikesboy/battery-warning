use std::{future::pending, process::ExitStatus, time::Duration};

use thiserror::Error;
use tokio::{
    process::{Child, Command},
    time::timeout,
};

use crate::{config::Config, policy::Level};

#[derive(Debug, Error)]
pub enum DialogError {
    #[error("cannot launch Zenity dialog")]
    Spawn(#[source] std::io::Error),
    #[error("cannot wait for Zenity")]
    Wait(#[source] std::io::Error),
    #[error("cannot terminate owned Zenity dialog")]
    Terminate(#[source] std::io::Error),
    #[error("timed out reaping Zenity")]
    ReapTimeout,
    #[error("Zenity failed: {0}")]
    UnexpectedExit(ExitStatus),
    #[error("previous dialog has not been reaped")]
    AlreadyOpen,
}

type Result<T> = std::result::Result<T, DialogError>;

#[derive(Default)]
pub struct Dialog {
    child: Option<Child>,
}

impl Dialog {
    pub fn show(&mut self, config: &Config, level: Level, percentage: f64) -> Result<()> {
        if self.child.is_some() {
            return Err(DialogError::AlreadyOpen);
        }
        let (title, heading, urgency, theme) = match level {
            Level::Low => ("Battery Low", "Battery low", "", &config.low_theme),
            Level::Critical => (
                "Battery Critical",
                "Battery critically low",
                "now ",
                &config.critical_theme,
            ),
        };
        let text = format!(
            "{heading}: {percentage}%.\n\nConnect your charger {urgency}to avoid losing your work."
        );
        self.child = Some(
            Command::new(&config.zenity)
                .args([
                    "--warning",
                    &format!("--title={title}"),
                    "--icon=battery-caution-symbolic",
                    &format!("--text={text}"),
                    "--ok-label=OK",
                    "--width=420",
                    "--no-wrap",
                ])
                .env(
                    "XDG_CONFIG_HOME",
                    std::path::absolute(theme).map_err(DialogError::Spawn)?,
                )
                .kill_on_drop(true)
                .spawn()
                .map_err(DialogError::Spawn)?,
        );
        eprintln!("issued {level:?} warning at {percentage}%");
        Ok(())
    }

    pub async fn completed(&mut self) -> Result<()> {
        let status = match &mut self.child {
            Some(child) => child.wait().await.map_err(DialogError::Wait)?,
            None => pending::<ExitStatus>().await,
        };
        self.child = None;
        if !matches!(status.code(), Some(0 | 1)) {
            return Err(DialogError::UnexpectedExit(status));
        }
        Ok(())
    }

    pub async fn close(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            // This wait belongs exclusively to the intentionally killed child. No old
            // completion callback can clear a subsequently spawned replacement.
            child.start_kill().map_err(DialogError::Terminate)?;
            timeout(Duration::from_secs(2), child.wait())
                .await
                .map_err(|_| DialogError::ReapTimeout)?
                .map_err(DialogError::Wait)?;
        }
        Ok(())
    }
}
