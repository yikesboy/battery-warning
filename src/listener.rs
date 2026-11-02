use std::future::Future;

use anyhow::{Context, Result};
use zbus::Connection;

use crate::{
    config::Config,
    dialog::Dialog,
    policy::{Effect, Policy},
    upower::UPower,
};

pub async fn run(
    connection: &Connection,
    config: &Config,
    shutdown: impl Future<Output = ()>,
) -> Result<()> {
    let mut dialog = Dialog::default();
    tokio::pin!(shutdown);
    let result = async {
        let mut power = tokio::select! {
            biased;
            () = &mut shutdown => return Ok(()),
            power = UPower::subscribe(connection) => power?,
        };
        let mut policy = Policy::new(config.thresholds())?;
        let mut initial = true;
        loop {
            let is_initial = initial;
            initial = false;
            // Keep an in-flight update alive if a child completes. Cancelling it
            // after consuming a signal could otherwise lose the final AC update.
            let update = async {
                if !is_initial {
                    power.changed().await?;
                }
                let snapshot = power.snapshot().await?;
                power.check_owner()?;
                Ok::<_, anyhow::Error>(snapshot)
            };
            tokio::pin!(update);
            let snapshot = loop {
                tokio::select! {
                    biased;
                    () = &mut shutdown => return Ok(()),
                    result = dialog.completed() => { result?; }
                    snapshot = &mut update => break snapshot.context("cannot read UPower state")?,
                }
            };
            match policy.evaluate(snapshot)? {
                Effect::None => {}
                Effect::Close => dialog.close().await?,
                Effect::Show(level) => {
                    dialog.close().await?;
                    dialog.show(config, level, snapshot.percentage)?;
                    policy.issued(level);
                }
            }
        }
    }
    .await;
    let cleanup = dialog.close().await;
    if let Err(error) = &cleanup {
        eprintln!("dialog cleanup failed: {error:#}");
    }
    result.and(cleanup.map_err(Into::into))
}
