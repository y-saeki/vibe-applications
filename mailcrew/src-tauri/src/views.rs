//! The custom views the user made: a name, which accounts, and a Gmail search.
//! They hold nothing secret, so they live in a JSON file in the app's config
//! directory rather than in the credential store.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{CommandError, ErrorKind, Result};

pub const FILE_NAME: &str = "views.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub id: String,
    pub name: String,
    /// The accounts the view shows; every account when empty.
    #[serde(default)]
    pub accounts: Vec<String>,
    /// A Gmail search, applied on top of the inbox; nothing when empty.
    #[serde(default)]
    pub query: String,
}

fn failed(what: &str, err: impl std::fmt::Display) -> CommandError {
    CommandError::new(
        ErrorKind::Internal,
        format!("ビューを{what}できませんでした: {err}"),
    )
}

/// The views saved at `path`, or none when nothing has been saved yet.
pub fn load(path: &Path) -> Result<Vec<View>> {
    match std::fs::read_to_string(path) {
        Ok(json) => serde_json::from_str(&json).map_err(|err| failed("読み込み", err)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(failed("読み込み", err)),
    }
}

/// Saves `views` at `path`, replacing what was there.
pub fn save(path: &Path, views: &[View]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|err| failed("保存", err))?;
    }
    let json = serde_json::to_string_pretty(views).map_err(|err| failed("保存", err))?;
    // Written beside the file and moved over it, so a crash halfway leaves the
    // old views rather than half of the new ones.
    let partial = path.with_extension("json.partial");
    std::fs::write(&partial, json).map_err(|err| failed("保存", err))?;
    std::fs::rename(&partial, path).map_err(|err| failed("保存", err))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mailcrew-views-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("nested").join(FILE_NAME)
    }

    #[test]
    fn nothing_saved_is_no_views() {
        assert_eq!(load(&scratch("empty")).unwrap(), Vec::new());
    }

    #[test]
    fn saved_views_load_back() {
        let path = scratch("round-trip");
        let views = vec![View {
            id: "v1".into(),
            name: "仕事".into(),
            accounts: vec!["a@x".into()],
            query: "is:unread".into(),
        }];
        save(&path, &views).unwrap();
        assert_eq!(load(&path).unwrap(), views);
        save(&path, &[]).unwrap();
        assert_eq!(load(&path).unwrap(), Vec::new());
    }

    #[test]
    fn missing_fields_mean_everything() {
        let path = scratch("defaults");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"[{"id":"v","name":"n"}]"#).unwrap();
        let views = load(&path).unwrap();
        assert!(views[0].accounts.is_empty());
        assert!(views[0].query.is_empty());
    }
}
