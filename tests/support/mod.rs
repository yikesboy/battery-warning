mod bus;
mod fixture;

pub(crate) use bus::{Battery, Bus, MissingPercentage, serve};
pub(crate) use fixture::Fixture;

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::time::timeout;

pub(crate) const DEADLINE: Duration = Duration::from_secs(5);

pub(crate) async fn wait_for(description: &str, mut ready: impl FnMut() -> bool) {
    timeout(DEADLINE, async {
        while !ready() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {description}"));
}

fn executable(name: &str) -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").expect("PATH"))
        .map(|dir| dir.join(name))
        .find(|path| {
            fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
        .unwrap_or_else(|| panic!("{name} not found in PATH"))
}

pub(crate) async fn gone(pid: u32) {
    wait_for("dialog to be reaped", || {
        !Path::new(&format!("/proc/{pid}")).exists()
    })
    .await;
}
