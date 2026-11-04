//! The user's choices that are not views or senders: for now, whether Mail
//! Crew looks up senders' BIMI logos. Kept in a JSON file in the app's config
//! directory, read once and then held in memory.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{CommandError, ErrorKind, Result};

pub const FILE_NAME: &str = "preferences.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    /// Look up the sender's BIMI record and mark certificate, which asks the
    /// sender's DNS and the certificate's host.
    pub brand_logos: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self { brand_logos: true }
    }
}

fn failed(what: &str, err: impl std::fmt::Display) -> CommandError {
    CommandError::new(
        ErrorKind::Internal,
        format!("設定を{what}できませんでした: {err}"),
    )
}

/// The preferences saved at `path`, or the defaults when none are.
pub fn load(path: &Path) -> Result<Preferences> {
    match std::fs::read_to_string(path) {
        Ok(json) => serde_json::from_str(&json).map_err(|err| failed("読み込み", err)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(err) => Err(failed("読み込み", err)),
    }
}

pub fn save(path: &Path, preferences: &Preferences) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|err| failed("保存", err))?;
    }
    let json = serde_json::to_string_pretty(preferences).map_err(|err| failed("保存", err))?;
    // Written beside the file and moved over it, so a crash halfway leaves the
    // old file rather than half of the new one.
    let partial = path.with_extension("json.partial");
    std::fs::write(&partial, json).map_err(|err| failed("保存", err))?;
    std::fs::rename(&partial, path).map_err(|err| failed("保存", err))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mailcrew-preferences-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(FILE_NAME)
    }

    #[test]
    fn logos_are_looked_up_until_the_user_says_otherwise() {
        let path = scratch("default");
        assert!(load(&path).unwrap().brand_logos);
        save(&path, &Preferences { brand_logos: false }).unwrap();
        assert!(!load(&path).unwrap().brand_logos);
        // A file from a version that knew fewer preferences keeps the defaults.
        std::fs::write(&path, "{}").unwrap();
        assert!(load(&path).unwrap().brand_logos);
    }
}
