use battery_notification_util::{config::Config, policy::Thresholds};
use clap::Parser;

#[test]
fn cli_custom_thresholds() {
    let config = Config::try_parse_from([
        "battery",
        "--zenity",
        "/bin/zenity",
        "--low-theme",
        "low",
        "--critical-theme",
        "critical",
        "--critical-threshold",
        "0",
        "--low-threshold",
        "12.5",
        "--rearm-threshold",
        "100",
    ])
    .unwrap();
    assert_eq!(
        config.thresholds(),
        Thresholds {
            critical: 0.0,
            low: 12.5,
            rearm: 100.0
        }
    );
    config.thresholds().validate().unwrap();
}

#[test]
fn cli_requires_paths_and_numeric_thresholds() {
    for args in [
        vec!["battery"],
        vec!["battery", "--zenity", "/bin/zenity", "--low-theme", "low"],
        vec![
            "battery",
            "--zenity",
            "/bin/zenity",
            "--critical-theme",
            "critical",
        ],
        vec![
            "battery",
            "--low-theme",
            "low",
            "--critical-theme",
            "critical",
        ],
        vec![
            "battery",
            "--zenity",
            "/bin/zenity",
            "--low-theme",
            "low",
            "--critical-theme",
            "critical",
            "--low-threshold",
            "invalid",
        ],
    ] {
        assert!(Config::try_parse_from(args).is_err());
    }
}
