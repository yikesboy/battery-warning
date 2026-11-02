use std::{
    collections::HashMap,
    fs,
    os::unix::fs::PermissionsExt,
    sync::{Arc, atomic::AtomicUsize},
};

use battery_notification_util::{config::Config, listener, upower};
use tempfile::TempDir;
use tokio::{sync::oneshot, task::JoinHandle, time::timeout};
use zbus::{Connection, fdo::RequestNameFlags};

use super::{Battery, Bus, DEADLINE, bus::Manager, executable, gone, serve, wait_for};

pub(crate) struct Launch {
    pub(crate) pid: u32,
    pub(crate) theme: String,
    pub(crate) args: Vec<String>,
}

pub(crate) struct Fixture {
    pub(crate) task: JoinHandle<anyhow::Result<()>>,
    shutdown: Option<oneshot::Sender<()>>,
    pub(crate) service: Connection,
    pub(crate) config: Config,
    pub(crate) reads: Arc<AtomicUsize>,
    dir: TempDir,
    pub(crate) _bus: Bus,
}

impl Fixture {
    pub(crate) async fn new(percentage: f64, outcome: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let config = Config {
            zenity: root.join("dialog"),
            low_theme: root.join("low"),
            critical_theme: root.join("critical"),
            low: 15.0,
            critical: 5.0,
            rearm: 20.0,
        };
        for theme in [&config.low_theme, &config.critical_theme] {
            fs::create_dir_all(theme.join("gtk-4.0")).unwrap();
            fs::write(theme.join("gtk-4.0/gtk.css"), "/* test theme */").unwrap();
        }
        // Each launch has its own atomic record. exec preserves the recorded PID
        // and leaves no shell subprocess behind when the listener kills it.
        fs::write(&config.zenity, format!(
            "#!{}\nroot=${{0%/*}}\nprintf '%s\\0' \"$$\" \"$XDG_CONFIG_HOME\" \"$@\" > \"$root/$$.tmp\"\nmv \"$root/$$.tmp\" \"$root/$$.launch\"\n{}\n",
            executable("sh").display(), outcome,
        )).unwrap();
        fs::set_permissions(&config.zenity, fs::Permissions::from_mode(0o700)).unwrap();
        config.validate().unwrap();
        let bus = Bus::new();
        let reads = Arc::new(AtomicUsize::new(0));
        let service = serve(&bus, percentage, reads.clone()).await;
        service
            .request_name_with_flags(upower::SERVICE, RequestNameFlags::AllowReplacement.into())
            .await
            .unwrap();
        let client = bus.connect().await;
        let (shutdown, receive) = oneshot::channel();
        // Move a separate Config into the task without changing the library API.
        let task_config = Config {
            zenity: config.zenity.clone(),
            low_theme: config.low_theme.clone(),
            critical_theme: config.critical_theme.clone(),
            low: config.low,
            critical: config.critical,
            rearm: config.rearm,
        };
        let task = tokio::spawn(async move {
            listener::run(&client, &task_config, async {
                let _ = receive.await;
            })
            .await
        });
        Self {
            task,
            shutdown: Some(shutdown),
            service,
            config,
            reads,
            dir,
            _bus: bus,
        }
    }

    pub(crate) fn launches(&self) -> Vec<Launch> {
        fs::read_dir(self.dir.path())
            .unwrap()
            .map(Result::unwrap)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "launch"))
            .map(|entry| {
                let text = fs::read_to_string(entry.path()).unwrap();
                let mut fields = text.strip_suffix('\0').unwrap().split('\0');
                Launch {
                    pid: fields.next().unwrap().parse().unwrap(),
                    theme: fields.next().unwrap().to_owned(),
                    args: fields.map(str::to_owned).collect(),
                }
            })
            .collect()
    }

    pub(crate) async fn launch(&self, count: usize, title: &str) -> Launch {
        wait_for("dialog launch", || self.launches().len() >= count).await;
        let launches = self.launches();
        assert_eq!(launches.len(), count);
        launches
            .into_iter()
            .find(|launch| launch.args.contains(&format!("--title={title}")))
            .unwrap_or_else(|| panic!("missing {title} dialog"))
    }

    pub(crate) async fn percentage(&self, value: f64, invalidate: bool) {
        let interface = self
            .service
            .object_server()
            .interface::<_, Battery>(upower::BATTERY)
            .await
            .unwrap();
        let mut battery = interface.get_mut().await;
        battery.percentage = value;
        if invalidate {
            zbus::fdo::Properties::properties_changed(
                interface.signal_emitter(),
                upower::DEVICE.try_into().unwrap(),
                HashMap::new(),
                vec!["Percentage"].into(),
            )
            .await
            .unwrap();
        } else {
            battery
                .percentage_changed(interface.signal_emitter())
                .await
                .unwrap();
        }
    }

    pub(crate) async fn on_battery(&self, value: bool) {
        let interface = self
            .service
            .object_server()
            .interface::<_, Manager>(upower::MANAGER)
            .await
            .unwrap();
        let mut manager = interface.get_mut().await;
        manager.on_battery = value;
        manager
            .on_battery_changed(interface.signal_emitter())
            .await
            .unwrap();
    }

    pub(crate) async fn finish(&mut self) -> anyhow::Result<()> {
        timeout(DEADLINE, &mut self.task)
            .await
            .expect("listener did not finish")
            .expect("listener panicked")
    }

    pub(crate) async fn stop(&mut self) {
        self.shutdown
            .take()
            .unwrap()
            .send(())
            .expect("listener exited before shutdown");
        self.finish().await.unwrap();
        for launch in self.launches() {
            gone(launch.pid).await;
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Aborting drops Dialog's kill_on_drop child even when an assertion fails.
        self.task.abort();
    }
}
