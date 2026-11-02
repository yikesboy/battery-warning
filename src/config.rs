use std::{fs, path::PathBuf};

use anyhow::{Context, Result, ensure};
use clap::Parser;

use crate::policy::Thresholds;

#[derive(Debug, Parser)]
#[command(
    name = "battery-warning",
    version,
    about = "Show acknowledgeable battery warnings with Zenity"
)]
pub struct Config {
    #[arg(long)]
    pub zenity: PathBuf,
    #[arg(long)]
    pub low_theme: PathBuf,
    #[arg(long)]
    pub critical_theme: PathBuf,
    #[arg(long = "low-threshold", default_value_t = 15.0)]
    pub low: f64,
    #[arg(long = "critical-threshold", default_value_t = 5.0)]
    pub critical: f64,
    #[arg(long = "rearm-threshold", default_value_t = 20.0)]
    pub rearm: f64,
}

impl Config {
    pub fn thresholds(&self) -> Thresholds {
        Thresholds {
            critical: self.critical,
            low: self.low,
            rearm: self.rearm,
        }
    }

    pub fn validate(&self) -> Result<()> {
        self.thresholds().validate()?;
        ensure!(
            self.zenity.is_absolute(),
            "--zenity must be an absolute path"
        );
        let metadata = fs::metadata(&self.zenity)
            .with_context(|| format!("cannot inspect --zenity {}", self.zenity.display()))?;
        ensure!(metadata.is_file(), "--zenity must point to a regular file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            ensure!(
                metadata.permissions().mode() & 0o111 != 0,
                "--zenity must have executable permissions"
            );
        }
        for (flag, theme) in [
            ("--low-theme", &self.low_theme),
            ("--critical-theme", &self.critical_theme),
        ] {
            ensure!(theme.is_dir(), "{flag} must point to a theme directory");
            let css = theme.join("gtk-4.0/gtk.css");
            let metadata = fs::metadata(&css)
                .with_context(|| format!("cannot inspect {flag} CSS {}", css.display()))?;
            ensure!(
                metadata.is_file(),
                "{flag} CSS must be a regular file: {}",
                css.display()
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                ensure!(
                    metadata.permissions().mode() & 0o444 != 0,
                    "{flag} CSS must have readable permissions: {}",
                    css.display()
                );
            }
            fs::File::open(&css)
                .with_context(|| format!("cannot read {flag} CSS {}", css.display()))?;
        }
        Ok(())
    }
}
