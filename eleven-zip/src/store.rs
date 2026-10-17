//! The folder eleven-zip keeps its own 7-Zip in (`%LOCALAPPDATA%\eleven-zip\7-Zip`).
//!
//! Each version gets a folder of its own, and a file named `current` says
//! which one to use. A new version is put in place beside the old and only
//! then made current, so that an extraction running from the old one is not
//! disturbed; the old folder is removed later, once nothing has it open.
//!
//! This is plain file handling, so it is tested on every platform.
//! win/sevenzip.rs downloads the package and unpacks it into a staging folder.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::release::Version;

/// The files eleven-zip runs. A version folder without both is not usable.
pub const CLI: &str = "7z.exe";
pub const GUI: &str = "7zG.exe";

const CURRENT: &str = "current";
const LAST_CHECK: &str = "last-check";
const LOCK: &str = "update.lock";
/// A lock older than this was left by an update that never finished.
const STALE_LOCK: Duration = Duration::from_secs(10 * 60);

/// A 7-Zip that is ready to run.
#[derive(Clone, Debug, PartialEq)]
pub struct Installed {
    pub version: Version,
    pub dir: PathBuf,
}

impl Installed {
    pub fn cli(&self) -> PathBuf {
        self.dir.join(CLI)
    }

    pub fn gui(&self) -> PathBuf {
        self.dir.join(GUI)
    }
}

pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: PathBuf) -> Store {
        Store { root }
    }

    #[cfg(test)]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The 7-Zip in use, if one is installed and its files are there.
    pub fn current(&self) -> Option<Installed> {
        let text = fs::read_to_string(self.root.join(CURRENT)).ok()?;
        let version: Version = text.parse().ok()?;
        let installed = Installed {
            version,
            dir: self.root.join(version.to_string()),
        };
        (installed.cli().is_file() && installed.gui().is_file()).then_some(installed)
    }

    /// An empty folder to download and unpack `version` in.
    pub fn staging(&self, version: Version) -> io::Result<PathBuf> {
        let dir = self.root.join(format!("staging-{version}"));
        match fs::remove_dir_all(&dir) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Moves an unpacked 7-Zip (`unpacked`, the folder with 7z.exe in it)
    /// into place as `version` and makes it the one in use.
    pub fn adopt(&self, version: Version, unpacked: &Path) -> io::Result<Installed> {
        for file in [CLI, GUI] {
            if !unpacked.join(file).is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("展開した 7-Zip に {file} がありません"),
                ));
            }
        }
        let dir = self.root.join(version.to_string());
        match fs::remove_dir_all(&dir) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        fs::rename(unpacked, &dir)?;
        // Written whole and then renamed over the old one, so that a reader
        // never sees half a version.
        let tmp = self.root.join(format!("{CURRENT}.tmp"));
        fs::write(&tmp, version.to_string())?;
        fs::rename(&tmp, self.root.join(CURRENT))?;
        Ok(Installed { version, dir })
    }

    /// Removes everything but the 7-Zip in use and the store's own files:
    /// older versions, and what interrupted updates left behind. Anything
    /// still open (an old version an extraction is running from) stays until
    /// the next time.
    pub fn prune(&self) {
        let keep = self.current().map(|c| c.dir);
        let Ok(entries) = fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if Some(&path) == keep.as_ref()
                || [CURRENT, LAST_CHECK, LOCK].map(Into::into).contains(&name)
            {
                continue;
            }
            let _ = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
        }
    }

    /// When eleven-zip last asked for a new version, in seconds since the Unix
    /// epoch.
    pub fn last_check(&self) -> Option<u64> {
        fs::read_to_string(self.root.join(LAST_CHECK))
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    pub fn set_last_check(&self, now: u64) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        fs::write(self.root.join(LAST_CHECK), now.to_string())
    }

    /// Claims the store for an update, or `None` while another eleven-zip is
    /// updating it. Explorer starts one eleven-zip per archive, so several may
    /// find an update due at once.
    pub fn lock(&self) -> io::Result<Option<Lock>> {
        fs::create_dir_all(&self.root)?;
        let path = self.root.join(LOCK);
        for _ in 0..2 {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => return Ok(Some(Lock(path))),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| SystemTime::now().duration_since(t).ok())
                        .is_some_and(|age| age > STALE_LOCK);
                    if !stale {
                        return Ok(None);
                    }
                    let _ = fs::remove_file(&path);
                }
                Err(e) => return Err(e),
            }
        }
        Ok(None)
    }
}

/// Held while updating; removing the file releases it.
pub struct Lock(PathBuf);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> Store {
        let root =
            std::env::temp_dir().join(format!("eleven-zip-store-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        Store::new(root)
    }

    /// A folder that looks like 7-Zip as the MSI unpacks it.
    fn unpacked(store: &Store, version: Version) -> PathBuf {
        let dir = store.staging(version).unwrap().join("Files").join("7-Zip");
        fs::create_dir_all(&dir).unwrap();
        for file in [CLI, GUI, "7z.dll"] {
            fs::write(dir.join(file), file).unwrap();
        }
        dir
    }

    fn v(s: &str) -> Version {
        s.parse().unwrap()
    }

    #[test]
    fn nothing_is_installed_at_first() {
        let store = temp_store("empty");
        assert_eq!(store.current(), None);
        assert_eq!(store.last_check(), None);
    }

    #[test]
    fn adopting_makes_a_version_current() {
        let store = temp_store("adopt");
        let installed = store
            .adopt(v("26.04"), &unpacked(&store, v("26.04")))
            .unwrap();
        assert_eq!(installed.dir, store.root().join("26.04"));
        assert_eq!(store.current(), Some(installed.clone()));
        assert!(installed.gui().is_file());
        let _ = fs::remove_dir_all(store.root());
    }

    #[test]
    fn a_package_without_7zip_in_it_is_refused() {
        let store = temp_store("refuse");
        let dir = unpacked(&store, v("26.04"));
        fs::remove_file(dir.join(GUI)).unwrap();
        assert!(store.adopt(v("26.04"), &dir).is_err());
        assert_eq!(store.current(), None);
        let _ = fs::remove_dir_all(store.root());
    }

    #[test]
    fn a_version_whose_files_are_gone_is_not_current() {
        let store = temp_store("gone");
        let installed = store
            .adopt(v("26.04"), &unpacked(&store, v("26.04")))
            .unwrap();
        fs::remove_file(installed.cli()).unwrap();
        assert_eq!(store.current(), None);
        let _ = fs::remove_dir_all(store.root());
    }

    #[test]
    fn pruning_keeps_only_the_current_version() {
        let store = temp_store("prune");
        store
            .adopt(v("25.01"), &unpacked(&store, v("25.01")))
            .unwrap();
        store
            .adopt(v("26.04"), &unpacked(&store, v("26.04")))
            .unwrap();
        store.set_last_check(5).unwrap();
        fs::write(store.root().join("7z2604-x64.msi"), "").unwrap();
        store.prune();
        let mut left: Vec<String> = fs::read_dir(store.root())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["26.04", "current", "last-check"]);
        assert_eq!(store.current().unwrap().version, v("26.04"));
        assert_eq!(store.last_check(), Some(5));
        let _ = fs::remove_dir_all(store.root());
    }

    #[test]
    fn one_update_at_a_time() {
        let store = temp_store("lock");
        let first = store.lock().unwrap();
        assert!(first.is_some());
        assert!(store.lock().unwrap().is_none());
        drop(first);
        assert!(store.lock().unwrap().is_some());
        let _ = fs::remove_dir_all(store.root());
    }
}
