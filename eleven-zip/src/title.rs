//! The settings window's title, and how a running one is recognised by it.
//!
//! The title carries the version, so it differs from one build to the next.
//! A second eleven-zip and the installer find the open window by what every
//! version's title starts with, not by the whole of it.
//!
//! Nothing here touches Win32, so it is tested on every platform.

/// What every version's title is or starts with. Up to 0.1.1 the title was
/// this alone; installer/installer.nsi closes the window by it.
const NAME: &str = "eleven-zip の設定";

/// The settings window's title, with the version (`0.1.2`).
pub fn window_title(version: &str) -> String {
    format!("{NAME} - v{version}")
}

/// Whether a window with this title is eleven-zip's settings window, of this
/// version or any other.
pub fn is_settings(title: &str) -> bool {
    title == NAME
        || title
            .strip_prefix(NAME)
            .is_some_and(|rest| rest.starts_with(" - v"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_shows_the_version() {
        assert_eq!(window_title("0.1.2"), "eleven-zip の設定 - v0.1.2");
    }

    #[test]
    fn any_versions_settings_window_is_recognised() {
        assert!(is_settings(&window_title("0.1.2")));
        assert!(is_settings(&window_title("10.0.0")));
        // 0.1.1 and earlier.
        assert!(is_settings("eleven-zip の設定"));
    }

    #[test]
    fn other_windows_are_not() {
        assert!(!is_settings(""));
        assert!(!is_settings("eleven-zip"));
        assert!(!is_settings("eleven-zip の設定のヘルプ"));
        assert!(!is_settings("メモ - eleven-zip の設定"));
    }
}
