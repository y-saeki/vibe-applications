//! Getting 7-Zip and keeping it up to date: asks GitHub for the newest
//! release, downloads its MSI package and unpacks it with `msiexec /a` (an
//! administrative install, which copies the files out without installing
//! anything or asking for administrator rights). store.rs decides where the
//! files go; release.rs reads GitHub's answer.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use super::{net, now};
use crate::release::{self, LATEST_URL, Release, Version};
use crate::store::{Installed, Store};

/// How long an extraction waits for another eleven-zip that is installing 7-Zip.
const WAIT_FOR_OTHER: Duration = Duration::from_secs(120);

#[derive(Clone, Debug, PartialEq)]
pub enum Checked {
    UpToDate(Version),
    Updated(Version),
    /// Another eleven-zip is updating right now.
    Busy,
}

/// The 7-Zip to extract with, downloading it first if there is none yet.
pub fn ensure(store: &Store) -> Result<Installed, String> {
    if let Some(installed) = store.current() {
        return Ok(installed);
    }
    if let Checked::Busy = update(store)? {
        let start = Instant::now();
        while start.elapsed() < WAIT_FOR_OTHER {
            std::thread::sleep(Duration::from_millis(500));
            if let Some(installed) = store.current() {
                return Ok(installed);
            }
        }
    }
    store
        .current()
        .ok_or_else(|| "7-Zip を用意できませんでした".to_string())
}

/// Installs the newest 7-Zip if it is newer than the one in use (or there
/// is none).
pub fn update(store: &Store) -> Result<Checked, String> {
    let Some(_lock) = store.lock().map_err(|e| e.to_string())? else {
        return Ok(Checked::Busy);
    };
    let release = latest()?;
    let _ = store.set_last_check(now());
    if let Some(current) = store.current()
        && current.version >= release.version
    {
        store.prune();
        return Ok(Checked::UpToDate(current.version));
    }
    let installed = install(store, &release)?;
    store.prune();
    Ok(Checked::Updated(installed.version))
}

/// Updates when automatic updates are on and the last check is a day old.
/// Run after an extraction, whose window is gone by then, so it says
/// nothing; a failure is tried again next time.
pub fn update_if_due(store: &Store, auto_update: bool) {
    if auto_update && release::is_due(store.last_check(), now()) {
        let _ = update(store);
    }
}

fn latest() -> Result<Release, String> {
    let body = net::get(LATEST_URL, Some("application/vnd.github+json"))?;
    release::parse_latest(&String::from_utf8_lossy(&body))
}

fn install(store: &Store, release: &Release) -> Result<Installed, String> {
    let staging = store.staging(release.version).map_err(|e| e.to_string())?;
    let result = (|| {
        let bytes = net::get(&release.msi_url, None)?;
        release::verify(release, &bytes)?;
        let msi = staging.join(release.version.msi_name());
        std::fs::write(&msi, bytes).map_err(|e| e.to_string())?;
        let target = staging.join("unpacked");
        unpack(&msi, &target)?;
        store
            .adopt(release.version, &target.join("Files").join("7-Zip"))
            .map_err(|e| e.to_string())
    })();
    let _ = std::fs::remove_dir_all(&staging);
    result
}

/// Copies the package's files under `target` (as `Files\7-Zip\...`).
fn unpack(msi: &Path, target: &Path) -> Result<(), String> {
    let system = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    // msiexec reads its own command line: the property wants its value in
    // quotes after the `=`, which Command's quoting of a whole argument
    // would not give it.
    let status = Command::new(system.join("System32").join("msiexec.exe"))
        .raw_arg(format!(
            "/a \"{}\" /qn TARGETDIR=\"{}\"",
            msi.display(),
            target.display()
        ))
        .status()
        .map_err(|e| format!("msiexec を起動できません: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "7-Zip のパッケージを展開できませんでした (msiexec {})",
            status.code().unwrap_or(-1)
        ))
    }
}
