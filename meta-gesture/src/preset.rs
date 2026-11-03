//! Presets: sets of commands made ready, with the keys each known
//! application takes for them. Adding a preset makes a set whose commands
//! remember which preset command they are, so that an application added to
//! the set later gets its keys filled in.
//!
//! The keys are those the applications' own documentation lists, read for
//! each platform; a command an application documents no keys for is left
//! without keys for it, rather than guessed:
//!
//! - Chrome: <https://support.google.com/chrome/answer/157179>
//! - Edge: <https://support.microsoft.com/en-us/edge/keyboard-shortcuts-in-microsoft-edge>
//! - Safari: <https://support.apple.com/guide/safari/keyboard-and-other-shortcuts-cpsh003/mac>
//! - Vivaldi: <https://help.vivaldi.com/desktop/tabs/tab-features/>, which lists only the keys
//!   that switch tabs; its keyboard shortcuts page sends the reader to the list in the app
//! - Firefox: its documentation (support.mozilla.org) answers with a check for robots that
//!   cannot be read here, so the keys are read from the keyset Firefox itself is built with,
//!   in Mozilla's repository:
//!   <https://github.com/mozilla-firefox/firefox/blob/main/browser/base/content/browser-sets.inc.xhtml>
//!   and the letters it names in `browser/locales/en-US/browser/browserSets.ftl`
//!
//! An application is recognized by its name as the settings window shows
//! it, which is the same on both platforms and for its beta and
//! developer builds, rather than by an id that differs between them.
//!
//! Nothing here touches the operating system, so it is tested on every
//! platform.

use crate::config::{App, Os};
use crate::gesture::Gesture;
use crate::keys::Combo;

/// A command of a preset.
pub struct Command {
    /// What a command made from it remembers (`config::Command::preset`).
    pub id: &'static str,
    pub name: &'static str,
    /// The gesture it starts with, which the user can change.
    pub gesture: &'static str,
}

/// An application a preset knows the keys of.
pub struct Application {
    pub label: &'static str,
    /// A word its name contains, compared without regard to case.
    word: &'static str,
    /// The keys for each command id, on Windows and on macOS. A command
    /// that is not listed has no documented keys there.
    windows: &'static [(&'static str, &'static str)],
    macos: &'static [(&'static str, &'static str)],
}

impl Application {
    /// Whether `app` is this application.
    pub fn is(&self, app: &App) -> bool {
        app.name.to_lowercase().contains(self.word)
    }

    /// The keys this application takes for the command `id` on `os`.
    pub fn keys(&self, os: Os, id: &str) -> Option<Combo> {
        let table = match os {
            Os::Windows => self.windows,
            Os::Macos => self.macos,
        };
        table
            .iter()
            .find(|(c, _)| *c == id)
            .map(|(_, k)| k.parse().expect("a preset's keys parse"))
    }
}

pub struct Preset {
    /// The name of the set it makes.
    pub name: &'static str,
    pub commands: &'static [Command],
    pub applications: &'static [Application],
}

impl Preset {
    /// The application of this preset that `app` is.
    pub fn application(&self, app: &App) -> Option<&Application> {
        self.applications.iter().find(|a| a.is(app))
    }
}

/// The browsers' preset.
pub const BROWSERS: Preset = Preset {
    name: "ブラウザ",
    commands: &[
        Command {
            id: "back",
            name: "戻る",
            gesture: "←",
        },
        Command {
            id: "forward",
            name: "進む",
            gesture: "→",
        },
        Command {
            id: "top",
            name: "ページの先頭へ",
            gesture: "↑",
        },
        Command {
            id: "bottom",
            name: "ページの末尾へ",
            gesture: "↓",
        },
        Command {
            id: "reload",
            name: "再読み込み",
            gesture: "↑↓",
        },
        Command {
            id: "new-tab",
            name: "新しいタブ",
            gesture: "↓↑",
        },
        Command {
            id: "close-tab",
            name: "タブを閉じる",
            gesture: "↓→",
        },
        Command {
            id: "reopen-tab",
            name: "閉じたタブを開き直す",
            gesture: "↓←",
        },
        Command {
            id: "previous-tab",
            name: "前のタブ",
            gesture: "↑←",
        },
        Command {
            id: "next-tab",
            name: "次のタブ",
            gesture: "↑→",
        },
    ],
    applications: &[
        Application {
            label: "Google Chrome",
            word: "chrome",
            windows: &[
                ("back", "Alt+Left"),
                ("forward", "Alt+Right"),
                ("top", "Home"),
                ("bottom", "End"),
                ("reload", "F5"),
                ("new-tab", "Ctrl+T"),
                ("close-tab", "Ctrl+W"),
                ("reopen-tab", "Ctrl+Shift+T"),
                ("previous-tab", "Ctrl+Shift+Tab"),
                ("next-tab", "Ctrl+Tab"),
            ],
            // ⌘[ and ⌘] are listed too; the arrows stay where they are on
            // every keyboard layout. The help lists no keys for reloading
            // or for the top and the bottom of the page.
            macos: &[
                ("back", "Cmd+Left"),
                ("forward", "Cmd+Right"),
                ("new-tab", "Cmd+T"),
                ("close-tab", "Cmd+W"),
                ("reopen-tab", "Cmd+Shift+T"),
                ("previous-tab", "Cmd+Option+Left"),
                ("next-tab", "Cmd+Option+Right"),
            ],
        },
        Application {
            label: "Microsoft Edge",
            word: "edge",
            windows: &[
                ("back", "Alt+Left"),
                ("forward", "Alt+Right"),
                ("top", "Home"),
                ("bottom", "End"),
                ("reload", "Ctrl+R"),
                ("new-tab", "Ctrl+T"),
                ("close-tab", "Ctrl+W"),
                ("reopen-tab", "Ctrl+Shift+T"),
                ("previous-tab", "Ctrl+Shift+Tab"),
                ("next-tab", "Ctrl+Tab"),
            ],
            // Command+Fn+Up and Down arrow switch tabs: Fn with an arrow is
            // Page Up and Page Down. No keys for the top and the bottom of
            // the page are listed.
            macos: &[
                ("back", "Cmd+Oem4"),
                ("forward", "Cmd+Oem6"),
                ("reload", "Cmd+R"),
                ("new-tab", "Cmd+T"),
                ("close-tab", "Cmd+W"),
                ("reopen-tab", "Cmd+Shift+T"),
                ("previous-tab", "Cmd+PageUp"),
                ("next-tab", "Cmd+PageDown"),
            ],
        },
        Application {
            label: "Firefox",
            word: "firefox",
            // `accel` is Ctrl on Windows and Command on macOS. The keys for
            // the top and the bottom of the page and for switching tabs are
            // not in the keyset.
            windows: &[
                ("back", "Alt+Left"),
                ("forward", "Alt+Right"),
                ("reload", "Ctrl+R"),
                ("new-tab", "Ctrl+T"),
                ("close-tab", "Ctrl+W"),
                ("reopen-tab", "Ctrl+Shift+T"),
            ],
            macos: &[
                ("back", "Cmd+Left"),
                ("forward", "Cmd+Right"),
                ("reload", "Cmd+R"),
                ("new-tab", "Cmd+T"),
                ("close-tab", "Cmd+W"),
                ("reopen-tab", "Cmd+Shift+T"),
            ],
        },
        Application {
            label: "Vivaldi",
            word: "vivaldi",
            // As the help writes them: the next tab with Ctrl+Page Up, the
            // previous with Ctrl+Page Down.
            windows: &[
                ("previous-tab", "Ctrl+PageDown"),
                ("next-tab", "Ctrl+PageUp"),
            ],
            macos: &[
                ("previous-tab", "Cmd+Shift+Oem4"),
                ("next-tab", "Cmd+Shift+Oem6"),
            ],
        },
        Application {
            label: "Safari",
            word: "safari",
            windows: &[],
            // No keys for reloading or for a new tab are listed.
            macos: &[
                ("back", "Cmd+Oem4"),
                ("forward", "Cmd+Oem6"),
                ("top", "Cmd+Up"),
                ("bottom", "Cmd+Down"),
                ("close-tab", "Cmd+W"),
                ("reopen-tab", "Cmd+Shift+T"),
                ("previous-tab", "Ctrl+Shift+Tab"),
                ("next-tab", "Ctrl+Tab"),
            ],
        },
    ],
};

/// The presets the settings window offers.
pub const ALL: [&Preset; 1] = [&BROWSERS];

impl Command {
    pub fn gesture(&self) -> Gesture {
        self.gesture.parse().expect("a preset's gesture parses")
    }
}

/// The preset command `id` of any preset.
pub fn command(id: &str) -> Option<(&'static Preset, &'static Command)> {
    ALL.iter()
        .flat_map(|p| p.commands.iter().map(move |c| (*p, c)))
        .find(|(_, c)| c.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str) -> App {
        App {
            id: "x".into(),
            name: name.into(),
        }
    }

    #[test]
    fn every_key_and_gesture_parses_and_no_gesture_is_used_twice() {
        for preset in ALL {
            for (i, c) in preset.commands.iter().enumerate() {
                c.gesture();
                assert!(
                    preset.commands[..i]
                        .iter()
                        .all(|o| o.gesture != c.gesture && o.id != c.id),
                    "{}",
                    c.id
                );
            }
            for a in preset.applications {
                for (id, keys) in a.windows.iter().chain(a.macos) {
                    assert!(preset.commands.iter().any(|c| c.id == *id), "{id}");
                    keys.parse::<Combo>().unwrap();
                }
            }
        }
    }

    #[test]
    fn recognizes_applications_by_name() {
        assert_eq!(
            BROWSERS.application(&app("Google Chrome")).unwrap().label,
            "Google Chrome"
        );
        assert_eq!(
            BROWSERS
                .application(&app("Google Chrome Beta"))
                .unwrap()
                .label,
            "Google Chrome"
        );
        assert_eq!(
            BROWSERS.application(&app("Microsoft Edge")).unwrap().label,
            "Microsoft Edge"
        );
        assert_eq!(
            BROWSERS.application(&app("Safari")).unwrap().label,
            "Safari"
        );
        assert_eq!(
            BROWSERS.application(&app("Firefox")).unwrap().label,
            "Firefox"
        );
        assert_eq!(
            BROWSERS.application(&app("Vivaldi")).unwrap().label,
            "Vivaldi"
        );
        assert!(BROWSERS.application(&app("Notepad")).is_none());
    }

    #[test]
    fn gives_each_platforms_documented_keys() {
        let chrome = BROWSERS.application(&app("Google Chrome")).unwrap();
        let k = |s: &str| s.parse::<Combo>().ok();
        assert_eq!(chrome.keys(Os::Windows, "back"), k("Alt+Left"));
        assert_eq!(chrome.keys(Os::Macos, "back"), k("Cmd+Left"));
        assert_eq!(chrome.keys(Os::Macos, "reload"), None);
        let safari = BROWSERS.application(&app("Safari")).unwrap();
        assert_eq!(safari.keys(Os::Windows, "back"), None);
        assert_eq!(command("close-tab").unwrap().1.name, "タブを閉じる");
        assert!(command("nothing").is_none());
    }
}
