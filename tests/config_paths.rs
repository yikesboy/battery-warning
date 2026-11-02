use std::{fs, path::PathBuf};

use battery_notification_util::{config::Config, policy::Thresholds};
use clap::Parser;
use tempfile::TempDir;

struct Fixture(TempDir);

impl Fixture {
    fn new() -> Self {
        let fixture = Self(TempDir::new().unwrap());
        fs::write(fixture.0.path().join("zenity"), b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                fixture.0.path().join("zenity"),
                fs::Permissions::from_mode(0o700),
            )
            .unwrap();
        }
        for theme in ["low", "critical"] {
            fs::create_dir_all(fixture.0.path().join(theme).join("gtk-4.0")).unwrap();
            fs::write(
                fixture.0.path().join(theme).join("gtk-4.0/gtk.css"),
                b"/* theme */",
            )
            .unwrap();
        }
        fixture
    }

    fn config(&self) -> Config {
        Config::try_parse_from([
            "battery-notification-util".into(),
            "--zenity".into(),
            self.0.path().join("zenity").into_os_string(),
            "--low-theme".into(),
            self.0.path().join("low").into_os_string(),
            "--critical-theme".into(),
            self.0.path().join("critical").into_os_string(),
        ])
        .unwrap()
    }
}

#[test]
fn cli_defaults_and_valid_paths() {
    let fixture = Fixture::new();
    let config = fixture.config();
    assert_eq!(
        config.thresholds(),
        Thresholds {
            critical: 5.0,
            low: 15.0,
            rearm: 20.0
        }
    );
    config.validate().unwrap();
}

#[test]
fn rejects_invalid_thresholds() {
    let fixture = Fixture::new();
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 101.0] {
        for field in 0..3 {
            let mut config = fixture.config();
            match field {
                0 => config.critical = invalid,
                1 => config.low = invalid,
                _ => config.rearm = invalid,
            }
            assert!(config.validate().is_err());
        }
    }
    let mut config = fixture.config();
    config.low = config.critical;
    assert!(config.validate().is_err());
    config.low = config.rearm;
    assert!(config.validate().is_err());
}

#[test]
fn rejects_relative_missing_or_directory_executable() {
    let fixture = Fixture::new();
    for path in [
        PathBuf::from("zenity"),
        fixture.0.path().join("missing"),
        fixture.0.path().to_path_buf(),
    ] {
        let mut config = fixture.config();
        config.zenity = path;
        assert!(config.validate().is_err());
    }
}

#[test]
fn rejects_missing_theme_directory_or_css_and_directory_css() {
    let fixture = Fixture::new();
    for critical in [false, true] {
        for path in [
            fixture.0.path().join("missing"),
            fixture.0.path().join("zenity"),
            fixture.0.path().to_path_buf(),
        ] {
            let mut config = fixture.config();
            if critical {
                config.critical_theme = path;
            } else {
                config.low_theme = path;
            }
            assert!(config.validate().is_err());
        }
        let config = fixture.config();
        let theme = if critical {
            &config.critical_theme
        } else {
            &config.low_theme
        };
        let css = theme.join("gtk-4.0/gtk.css");
        fs::remove_file(&css).unwrap();
        assert!(config.validate().is_err());
        fs::create_dir(&css).unwrap();
        assert!(config.validate().is_err());
        fs::remove_dir(&css).unwrap();
        fs::write(&css, b"").unwrap();
    }
}

#[cfg(unix)]
#[test]
fn rejects_non_executable_zenity_and_unreadable_css() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let config = fixture.config();
    fs::set_permissions(&config.zenity, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(config.validate().is_err());
    fs::set_permissions(&config.zenity, fs::Permissions::from_mode(0o700)).unwrap();
    for theme in [&config.low_theme, &config.critical_theme] {
        let css = theme.join("gtk-4.0/gtk.css");
        fs::set_permissions(&css, fs::Permissions::from_mode(0o000)).unwrap();
        assert!(config.validate().is_err());
        fs::set_permissions(&css, fs::Permissions::from_mode(0o600)).unwrap();
    }
    config.validate().unwrap();
}
