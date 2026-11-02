#![cfg(target_os = "linux")]

mod support;
use support::*;

use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use battery_notification_util::upower;
use tokio::time::timeout;

#[path = "cases/lifecycle.rs"]
mod lifecycle;

#[tokio::test]
async fn missing_required_property_fails_initial_snapshot() {
    let bus = Bus::new();
    let service = serve(&bus, 25.0, Arc::new(AtomicUsize::new(0))).await;
    service
        .object_server()
        .remove::<Battery, _>(upower::BATTERY)
        .await
        .unwrap();
    service
        .object_server()
        .at(upower::BATTERY, MissingPercentage)
        .await
        .unwrap();
    service.request_name(upower::SERVICE).await.unwrap();
    let client = bus.connect().await;
    let error = timeout(DEADLINE, async {
        let power = upower::UPower::subscribe(&client).await.unwrap();
        power
            .snapshot()
            .await
            .expect_err("missing property accepted")
    })
    .await
    .expect("initial snapshot did not finish");
    assert!(format!("{error:#}").contains("Percentage"), "{error:#}");
}

#[tokio::test]
async fn unavailable_upower_fails_subscription() {
    let bus = Bus::new();
    let client = bus.connect().await;
    let error = timeout(DEADLINE, upower::UPower::subscribe(&client))
        .await
        .expect("subscription did not finish")
        .err()
        .expect("unavailable UPower accepted");
    assert!(
        format!("{error:#}").contains("UPower is unavailable"),
        "{error:#}"
    );
}

#[tokio::test]
async fn healthy_startup_does_not_warn() {
    let mut fixture = Fixture::new(25.0, "exec sleep 300").await;
    wait_for("initial healthy snapshot", || {
        fixture.reads.load(Ordering::SeqCst) > 0
    })
    .await;
    assert!(!fixture.task.is_finished());
    fixture.stop().await;
    assert!(fixture.launches().is_empty());
}

#[tokio::test]
async fn spawn_failure_after_healthy_startup_is_an_error() {
    let mut fixture = Fixture::new(25.0, "exec sleep 300").await;
    wait_for("initial healthy snapshot", || {
        fixture.reads.load(Ordering::SeqCst) > 0
    })
    .await;
    fs::remove_file(&fixture.config.zenity).unwrap();
    fixture.percentage(15.0, false).await;
    let error = fixture.finish().await.unwrap_err();
    assert!(
        format!("{error:#}").contains("cannot launch Zenity dialog"),
        "{error:#}"
    );
    assert!(fixture.launches().is_empty());
}

#[tokio::test]
async fn nan_update_fails_and_reaps_dialog() {
    let mut fixture = Fixture::new(15.0, "exec sleep 300").await;
    let launch = fixture.launch(1, "Battery Low").await;
    fixture.percentage(f64::NAN, false).await;
    let error = fixture.finish().await.unwrap_err();
    assert!(format!("{error:#}").contains("percentage"), "{error:#}");
    gone(launch.pid).await;
    assert_eq!(fixture.launches().len(), 1);
}

#[tokio::test]
async fn initial_low_and_critical_dialog_arguments_and_shutdown() {
    for (percentage, title, heading, urgency) in [
        (15.0, "Battery Low", "Battery low", ""),
        (5.0, "Battery Critical", "Battery critically low", "now "),
    ] {
        let mut fixture = Fixture::new(percentage, "exec sleep 300").await;
        let launch = fixture.launch(1, title).await;
        let theme = if percentage == 5.0 {
            &fixture.config.critical_theme
        } else {
            &fixture.config.low_theme
        };
        assert_eq!(launch.theme, theme.to_str().unwrap());
        assert_eq!(
            launch.args,
            [
                "--warning".to_owned(),
                format!("--title={title}"),
                "--icon=battery-caution-symbolic".to_owned(),
                format!(
                    "--text={heading}: {percentage}%.\n\nConnect your charger {urgency}to avoid losing your work."
                ),
                "--ok-label=OK".to_owned(),
                "--width=420".to_owned(),
                "--no-wrap".to_owned(),
            ]
        );
        fixture.stop().await;
        assert_eq!(fixture.launches().len(), 1);
    }
}
