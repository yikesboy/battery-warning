use super::*;
use zbus::fdo::RequestNameFlags;

#[tokio::test]
async fn bus_connection_loss_fails_and_reaps_dialog() {
    let mut fixture = Fixture::new(15.0, "exec sleep 300").await;
    let launch = fixture.launch(1, "Battery Low").await;
    fixture._bus.child.kill().unwrap();
    fixture._bus.child.wait().unwrap();
    let error = fixture.finish().await.unwrap_err();
    assert!(
        format!("{error:#}").contains("cannot read UPower state"),
        "{error:#}"
    );
    gone(launch.pid).await;
    assert_eq!(fixture.launches().len(), 1);
}

#[tokio::test]
async fn ac_dismisses_and_rearms_warning() {
    let mut fixture = Fixture::new(15.0, "exec sleep 300").await;
    let low = fixture.launch(1, "Battery Low").await;
    fixture.on_battery(false).await;
    gone(low.pid).await;
    assert!(!fixture.task.is_finished());
    fixture.on_battery(true).await;
    fixture.launch(2, "Battery Low").await;
    fixture.stop().await;
}

#[tokio::test]
async fn escalation_reaps_low_and_keeps_critical_owned() {
    let mut fixture = Fixture::new(15.0, "exec sleep 300").await;
    let low = fixture.launch(1, "Battery Low").await;
    fixture.percentage(5.0, false).await;
    let critical = fixture.launch(2, "Battery Critical").await;
    gone(low.pid).await;
    assert_ne!(critical.pid, low.pid);
    assert_eq!(
        critical.theme,
        fixture.config.critical_theme.to_str().unwrap()
    );
    fixture.on_battery(false).await;
    gone(critical.pid).await;
    fixture.stop().await;
}

#[tokio::test]
async fn acknowledgement_zero_and_one_do_not_repeat_low_but_allow_escalation() {
    for code in [0, 1] {
        let mut fixture = Fixture::new(15.0, &format!("exit {code}")).await;
        let low = fixture.launch(1, "Battery Low").await;
        gone(low.pid).await;
        assert!(!fixture.task.is_finished());
        let reads = fixture.reads.load(Ordering::SeqCst);
        fixture.percentage(14.0, false).await;
        wait_for("acknowledged low snapshot", || {
            fixture.reads.load(Ordering::SeqCst) > reads
        })
        .await;
        fixture.percentage(5.0, false).await;
        let critical = fixture.launch(2, "Battery Critical").await;
        gone(critical.pid).await;
        fixture.stop().await;
        assert_eq!(fixture.launches().len(), 2);
    }
}

#[tokio::test]
async fn child_exit_127_and_signal_are_errors() {
    for (outcome, status) in [("exit 127", "127"), ("kill -TERM $$", "signal: 15")] {
        let mut fixture = Fixture::new(15.0, outcome).await;
        let launch = fixture.launch(1, "Battery Low").await;
        let error = fixture.finish().await.unwrap_err();
        assert!(format!("{error:#}").contains("Zenity failed"), "{error:#}");
        assert!(format!("{error:#}").contains(status), "{error:#}");
        gone(launch.pid).await;
    }
}

#[tokio::test]
async fn owner_disappearance_and_replacement_fail_and_reap_dialog() {
    for replace in [false, true] {
        let mut fixture = Fixture::new(15.0, "exec sleep 300").await;
        let launch = fixture.launch(1, "Battery Low").await;
        let replacement = if replace {
            let connection = serve(&fixture._bus, 5.0, Arc::new(AtomicUsize::new(0))).await;
            connection
                .request_name_with_flags(upower::SERVICE, RequestNameFlags::ReplaceExisting.into())
                .await
                .unwrap();
            Some(connection)
        } else {
            assert!(fixture.service.release_name(upower::SERVICE).await.unwrap());
            None
        };
        let error = fixture.finish().await.unwrap_err();
        assert!(
            format!("{error:#}").contains("UPower disappeared or its bus owner changed"),
            "{error:#}"
        );
        gone(launch.pid).await;
        assert_eq!(fixture.launches().len(), 1);
        drop(replacement);
    }
}

#[tokio::test]
async fn invalidated_percentage_is_refetched_for_escalation() {
    let mut fixture = Fixture::new(15.0, "exec sleep 300").await;
    let low = fixture.launch(1, "Battery Low").await;
    fixture.percentage(4.0, true).await;
    let critical = fixture.launch(2, "Battery Critical").await;
    assert!(
        critical
            .args
            .iter()
            .any(|arg| arg.starts_with("--text=Battery critically low: 4%.\n"))
    );
    gone(low.pid).await;
    fixture.stop().await;
}
