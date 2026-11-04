//! The senders whose messages show images from the network without asking,
//! which the user chose with この差出人は常に表示. Gmail remembers the same
//! choice by sender address, and so does this: a whole domain would let in
//! everyone at gmail.com once one of them was trusted.
//!
//! The addresses hold nothing secret, so they live in a JSON file in the app's
//! config directory beside the views.

use std::path::Path;

use crate::error::{CommandError, ErrorKind, Result};

pub const FILE_NAME: &str = "image-senders.json";

fn failed(what: &str, err: impl std::fmt::Display) -> CommandError {
    CommandError::new(
        ErrorKind::Internal,
        format!("画像を表示する差出人を{what}できませんでした: {err}"),
    )
}

/// Addresses are compared without case, so they are kept in lower case, once
/// each and in the order they were first given.
pub fn normalize(senders: &[String]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for sender in senders {
        let sender = sender.trim().to_lowercase();
        if !sender.is_empty() && !kept.contains(&sender) {
            kept.push(sender);
        }
    }
    kept
}

/// The senders saved at `path`, or none when nothing has been saved yet.
pub fn load(path: &Path) -> Result<Vec<String>> {
    match std::fs::read_to_string(path) {
        Ok(json) => serde_json::from_str::<Vec<String>>(&json)
            .map(|senders| normalize(&senders))
            .map_err(|err| failed("読み込み", err)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(failed("読み込み", err)),
    }
}

/// Saves `senders` at `path`, replacing what was there, and returns what was
/// saved.
pub fn save(path: &Path, senders: &[String]) -> Result<Vec<String>> {
    let senders = normalize(senders);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|err| failed("保存", err))?;
    }
    let json = serde_json::to_string_pretty(&senders).map_err(|err| failed("保存", err))?;
    // Written beside the file and moved over it, so a crash halfway leaves the
    // old list rather than half of the new one.
    let partial = path.with_extension("json.partial");
    std::fs::write(&partial, json).map_err(|err| failed("保存", err))?;
    std::fs::rename(&partial, path).map_err(|err| failed("保存", err))?;
    Ok(senders)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mailcrew-image-senders-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("nested").join(FILE_NAME)
    }

    #[test]
    fn nothing_saved_is_no_senders() {
        assert_eq!(load(&scratch("empty")).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn saved_senders_load_back() {
        let path = scratch("round-trip");
        let senders = vec!["news@example.com".to_string(), "a@b.jp".to_string()];
        assert_eq!(save(&path, &senders).unwrap(), senders);
        assert_eq!(load(&path).unwrap(), senders);
        save(&path, &[]).unwrap();
        assert_eq!(load(&path).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn addresses_are_kept_once_in_lower_case() {
        let given = [" News@Example.com ", "news@example.com", "", "b@x.jp"].map(String::from);
        assert_eq!(normalize(&given), ["news@example.com", "b@x.jp"]);
    }
}
