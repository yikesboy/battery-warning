use anyhow::{Context, Result, bail, ensure};
use futures_util::{FutureExt, StreamExt};
use zbus::{Connection, MatchRule, MessageStream, Proxy, proxy::CacheProperties};

use crate::policy::Snapshot;

pub const SERVICE: &str = "org.freedesktop.UPower";
pub const MANAGER: &str = "/org/freedesktop/UPower";
pub const BATTERY: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
pub const DEVICE: &str = "org.freedesktop.UPower.Device";

pub struct UPower<'a> {
    manager: Proxy<'a>,
    battery: Proxy<'a>,
    owners: MessageStream,
    manager_changes: MessageStream,
    battery_changes: MessageStream,
}

impl<'a> UPower<'a> {
    pub async fn subscribe(connection: &'a Connection) -> Result<Self> {
        // Await AddMatch, including its bus reply, before resolving the owner or
        // reading properties. An owner transition during setup stays queued.
        let owners = MessageStream::for_match_rule(
            MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender("org.freedesktop.DBus")?
                .path("/org/freedesktop/DBus")?
                .interface("org.freedesktop.DBus")?
                .member("NameOwnerChanged")?
                .add_arg(SERVICE)?
                .build(),
            connection,
            None,
        )
        .await?;
        let dbus = zbus::fdo::DBusProxy::new(connection).await?;
        let owner = dbus
            .get_name_owner(SERVICE.try_into()?)
            .await
            .context("UPower is unavailable")?;
        let mut streams = Vec::new();
        for (path, interface) in [(MANAGER, SERVICE), (BATTERY, DEVICE)] {
            streams.push(
                MessageStream::for_match_rule(
                    MatchRule::builder()
                        .msg_type(zbus::message::Type::Signal)
                        .sender(owner.clone())?
                        .path(path)?
                        .interface("org.freedesktop.DBus.Properties")?
                        .member("PropertiesChanged")?
                        .add_arg(interface)?
                        .build(),
                    connection,
                    None,
                )
                .await?,
            );
        }
        let manager = zbus::proxy::Builder::new(connection)
            .destination(owner.clone())?
            .path(MANAGER)?
            .interface(SERVICE)?
            .cache_properties(CacheProperties::No)
            .build()
            .await?;
        let battery = zbus::proxy::Builder::new(connection)
            .destination(owner.clone())?
            .path(BATTERY)?
            .interface(DEVICE)?
            .cache_properties(CacheProperties::No)
            .build()
            .await?;
        ensure!(
            dbus.get_name_owner(SERVICE.try_into()?).await? == owner,
            "UPower owner changed during setup"
        );
        Ok(Self {
            manager,
            battery,
            owners,
            battery_changes: streams.pop().unwrap(),
            manager_changes: streams.pop().unwrap(),
        })
    }

    pub async fn snapshot(&self) -> Result<Snapshot> {
        // Always fetch typed properties, including invalidated values; no stale
        // zbus cache or default values can conceal a missing property.
        Ok(Snapshot {
            on_battery: self.manager.get_property("OnBattery").await?,
            present: self.battery.get_property("IsPresent").await?,
            percentage: self.battery.get_property("Percentage").await?,
            state: self.battery.get_property("State").await?,
        })
    }

    pub async fn changed(&mut self) -> Result<()> {
        tokio::select! {
            biased;
            owner = self.owners.next() => {
                owner.context("D-Bus owner stream ended")??;
                bail!("UPower disappeared or its bus owner changed");
            }
            change = self.manager_changes.next() => { change.context("UPower manager stream ended")??; }
            change = self.battery_changes.next() => { change.context("UPower battery stream ended")??; }
        }
        Ok(())
    }

    pub fn check_owner(&mut self) -> Result<()> {
        if let Some(event) = self.owners.next().now_or_never() {
            event.context("D-Bus owner stream ended")??;
            bail!("UPower disappeared or its bus owner changed");
        }
        Ok(())
    }
}
