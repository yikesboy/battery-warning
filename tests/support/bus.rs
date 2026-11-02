use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use battery_notification_util::upower;
use zbus::{Connection, connection::Builder};

use super::DEADLINE;

pub(crate) struct Bus {
    pub(crate) child: Child,
    address: String,
}

impl Bus {
    pub(crate) fn new() -> Self {
        let config = std::env::var("BATTERY_TEST_DBUS_CONFIG")
            .map(|path| format!("--config-file={path}"))
            .unwrap_or_else(|_| "--session".to_owned());
        let child = Command::new("dbus-daemon")
            .args([config.as_str(), "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("start private dbus-daemon");
        // Install the guard before reading so a failed startup also gets reaped.
        let mut bus = Self {
            child,
            address: String::new(),
        };
        let stdout = bus.child.stdout.take().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut address = String::new();
            let result = BufReader::new(stdout).read_line(&mut address);
            let _ = send.send(result.map(|_| address));
        });
        bus.address = receive
            .recv_timeout(DEADLINE)
            .expect("timed out reading dbus-daemon address")
            .expect("read dbus-daemon address");
        bus.address = bus.address.trim().to_owned();
        assert!(
            !bus.address.is_empty(),
            "dbus-daemon did not print an address"
        );
        bus
    }

    pub(crate) async fn connect(&self) -> Connection {
        Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub(super) struct Manager {
    pub(super) on_battery: bool,
}

#[zbus::interface(name = "org.freedesktop.UPower")]
impl Manager {
    #[zbus(property)]
    fn on_battery(&self) -> bool {
        self.on_battery
    }
}

pub(crate) struct Battery {
    present: bool,
    pub(super) percentage: f64,
    state: u32,
    reads: Arc<AtomicUsize>,
}

pub(crate) struct MissingPercentage;

#[zbus::interface(name = "org.freedesktop.UPower.Device")]
impl MissingPercentage {
    #[zbus(property)]
    fn is_present(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn state(&self) -> u32 {
        2
    }
}

#[zbus::interface(name = "org.freedesktop.UPower.Device")]
impl Battery {
    #[zbus(property)]
    fn is_present(&self) -> bool {
        self.present
    }

    #[zbus(property)]
    fn percentage(&self) -> f64 {
        self.percentage
    }

    #[zbus(property)]
    fn state(&self) -> u32 {
        // State is the last property fetched in each listener snapshot.
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.state
    }
}

pub(crate) async fn serve(bus: &Bus, percentage: f64, reads: Arc<AtomicUsize>) -> Connection {
    Builder::address(bus.address.as_str())
        .unwrap()
        .serve_at(upower::MANAGER, Manager { on_battery: true })
        .unwrap()
        .serve_at(
            upower::BATTERY,
            Battery {
                present: true,
                percentage,
                state: 2,
                reads,
            },
        )
        .unwrap()
        .build()
        .await
        .unwrap()
}
