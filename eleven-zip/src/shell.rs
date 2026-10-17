//! What eleven-zip writes to the registry to appear in Explorer, and what it
//! removes again.
//!
//! Everything is under HKEY_CURRENT_USER, so neither needs administrator
//! rights:
//!
//! - A ProgID, `eleven-zip.archive`, whose `open` extracts. Each archive
//!   extension lists it under `OpenWithProgids`, so that eleven-zip is offered in
//!   「プログラムから開く」 and can be made the default there. Windows lets only
//!   the user choose a default, so eleven-zip never sets one itself.
//! - The same extensions as the application's capabilities, under
//!   `RegisteredApplications`, so that Windows' default-apps settings have a
//!   page for eleven-zip.
//! - With the context menu on, a verb eleven-zip で展開 for each extension under
//!   `SystemFileAssociations`, which Explorer shows whichever application
//!   opens the file.
//!
//! Nothing here touches Win32, so it is tested on every platform;
//! win/registry.rs applies the plan.

use crate::cli;

pub const APP_NAME: &str = "eleven-zip";
pub const PROG_ID: &str = "eleven-zip.archive";
const VERB: &str = "eleven-zip.extract";
const VERB_LABEL: &str = "eleven-zip で展開";
const TYPE_NAME: &str = "圧縮ファイル";
const DESCRIPTION: &str = "7-Zip で書庫を展開します";
const CAPABILITIES: &str = r"Software\eleven-zip\Capabilities";
/// The key this application owns outright, `Capabilities` among it.
const APP_KEY: &str = r"Software\eleven-zip";
const REGISTERED_APPLICATIONS: &str = r"Software\RegisteredApplications";

/// The extensions eleven-zip extracts, without the dot. All are formats 7-Zip
/// reads; `001` is the first volume of a split archive.
pub const EXTENSIONS: [&str; 22] = [
    "7z", "zip", "rar", "tar", "gz", "tgz", "bz2", "tbz", "tbz2", "xz", "txz", "zst", "tzst", "lz",
    "lzma", "lzh", "lha", "cab", "iso", "arj", "z", "001",
];

/// A string value: `name` empty is the key's default value.
#[derive(Clone, Debug, PartialEq)]
pub struct Value {
    pub key: String,
    pub name: String,
    pub data: String,
}

/// Changes to make under HKEY_CURRENT_USER, in this order: keys removed with
/// everything under them, single values removed from keys others share, and
/// values written (creating their keys).
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    pub remove_keys: Vec<String>,
    pub remove_values: Vec<(String, String)>,
    pub set: Vec<Value>,
}

fn value(key: impl Into<String>, name: &str, data: impl Into<String>) -> Value {
    Value {
        key: key.into(),
        name: name.to_string(),
        data: data.into(),
    }
}

fn classes(path: &str) -> String {
    format!(r"Software\Classes\{path}")
}

fn verb_key(ext: &str) -> String {
    classes(&format!(r"SystemFileAssociations\.{ext}\shell\{VERB}"))
}

/// The keys only eleven-zip writes, which go whole.
fn owned_keys() -> Vec<String> {
    vec![
        classes(PROG_ID),
        classes(r"Applications\eleven-zip.exe"),
        APP_KEY.to_string(),
    ]
}

/// The single values eleven-zip adds to keys other applications use too.
fn shared_values() -> Vec<(String, String)> {
    EXTENSIONS
        .iter()
        .map(|ext| {
            (
                classes(&format!(r".{ext}\OpenWithProgids")),
                PROG_ID.to_string(),
            )
        })
        .chain([(REGISTERED_APPLICATIONS.to_string(), APP_NAME.to_string())])
        .collect()
}

/// Everything eleven-zip needs in the registry, for the executable at `exe`.
/// The context menu's verbs are added or, with `context_menu` off, removed.
pub fn register(exe: &str, context_menu: bool) -> Plan {
    let command = format!("\"{exe}\" {} \"%1\"", cli::EXTRACT);
    let icon = format!("{exe},0");
    let mut set = vec![
        value(classes(PROG_ID), "", TYPE_NAME),
        value(classes(&format!(r"{PROG_ID}\DefaultIcon")), "", &icon),
        value(
            classes(&format!(r"{PROG_ID}\shell\open")),
            "FriendlyAppName",
            APP_NAME,
        ),
        value(
            classes(&format!(r"{PROG_ID}\shell\open\command")),
            "",
            &command,
        ),
        value(
            classes(r"Applications\eleven-zip.exe"),
            "FriendlyAppName",
            APP_NAME,
        ),
        value(CAPABILITIES, "ApplicationName", APP_NAME),
        value(CAPABILITIES, "ApplicationDescription", DESCRIPTION),
        value(REGISTERED_APPLICATIONS, APP_NAME, CAPABILITIES),
    ];
    for ext in EXTENSIONS {
        set.push(value(
            classes(&format!(r".{ext}\OpenWithProgids")),
            PROG_ID,
            "",
        ));
        set.push(value(
            format!(r"{CAPABILITIES}\FileAssociations"),
            &format!(".{ext}"),
            PROG_ID,
        ));
    }
    let verbs = EXTENSIONS.iter().map(|ext| verb_key(ext));
    let mut remove_keys = Vec::new();
    if context_menu {
        for key in verbs {
            set.push(value(&key, "MUIVerb", VERB_LABEL));
            set.push(value(&key, "Icon", &icon));
            set.push(value(format!(r"{key}\command"), "", &command));
        }
    } else {
        remove_keys.extend(verbs);
    }
    Plan {
        remove_keys,
        remove_values: Vec::new(),
        set,
    }
}

/// Removes everything [`register`] writes.
pub fn unregister() -> Plan {
    Plan {
        remove_keys: owned_keys()
            .into_iter()
            .chain(EXTENSIONS.iter().map(|ext| verb_key(ext)))
            .collect(),
        remove_values: shared_values(),
        set: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = r"C:\Users\a b\AppData\Local\eleven-zip\eleven-zip.exe";

    fn find<'a>(plan: &'a Plan, key: &str, name: &str) -> Option<&'a str> {
        plan.set
            .iter()
            .find(|v| v.key == key && v.name == name)
            .map(|v| v.data.as_str())
    }

    #[test]
    fn opening_an_archive_extracts_it() {
        let plan = register(EXE, true);
        assert_eq!(
            find(
                &plan,
                r"Software\Classes\eleven-zip.archive\shell\open\command",
                ""
            ),
            Some(r#""C:\Users\a b\AppData\Local\eleven-zip\eleven-zip.exe" --extract "%1""#)
        );
        assert_eq!(
            find(
                &plan,
                r"Software\Classes\.zip\OpenWithProgids",
                "eleven-zip.archive"
            ),
            Some("")
        );
        assert_eq!(
            find(
                &plan,
                r"Software\eleven-zip\Capabilities\FileAssociations",
                ".7z"
            ),
            Some("eleven-zip.archive")
        );
        assert_eq!(
            find(&plan, r"Software\RegisteredApplications", "eleven-zip"),
            Some(r"Software\eleven-zip\Capabilities")
        );
    }

    #[test]
    fn the_context_menu_verb() {
        let plan = register(EXE, true);
        let key = r"Software\Classes\SystemFileAssociations\.rar\shell\eleven-zip.extract";
        assert_eq!(find(&plan, key, "MUIVerb"), Some("eleven-zip で展開"));
        assert_eq!(
            find(&plan, &format!(r"{key}\command"), ""),
            find(
                &plan,
                r"Software\Classes\eleven-zip.archive\shell\open\command",
                ""
            )
        );
        assert!(plan.remove_keys.is_empty());
    }

    #[test]
    fn turning_the_context_menu_off_removes_its_verbs_and_nothing_else() {
        let plan = register(EXE, false);
        assert!(
            plan.set
                .iter()
                .all(|v| !v.key.contains("SystemFileAssociations"))
        );
        assert_eq!(plan.remove_keys.len(), EXTENSIONS.len());
        assert!(
            plan.remove_keys
                .iter()
                .all(|k| k.ends_with(r"\shell\eleven-zip.extract"))
        );
        // The ProgID stays: double-clicking still extracts.
        assert!(
            find(
                &plan,
                r"Software\Classes\eleven-zip.archive\shell\open\command",
                ""
            )
            .is_some()
        );
    }

    #[test]
    fn unregistering_removes_everything_registering_writes() {
        let undo = unregister();
        for v in register(EXE, true).set {
            let key_removed = undo
                .remove_keys
                .iter()
                .any(|k| v.key == *k || v.key.starts_with(&format!(r"{k}\")));
            let value_removed = undo
                .remove_values
                .contains(&(v.key.clone(), v.name.clone()));
            assert!(key_removed || value_removed, "{v:?} is left behind");
        }
    }

    #[test]
    fn unregistering_leaves_keys_others_share() {
        let undo = unregister();
        for key in &undo.remove_keys {
            assert!(
                key.contains("eleven-zip"),
                "{key} is not eleven-zip's to remove"
            );
        }
    }

    #[test]
    fn extensions_are_distinct_and_bare() {
        let mut seen = std::collections::HashSet::new();
        for ext in EXTENSIONS {
            assert!(!ext.starts_with('.') && ext == ext.to_lowercase(), "{ext}");
            assert!(seen.insert(ext), "{ext}");
        }
    }
}
