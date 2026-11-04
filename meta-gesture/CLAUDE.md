# meta-gesture

Repository-wide conventions are in the root `CLAUDE.md`; these apply on top of them.

## Documentation layout

`README.md` is for people who install and use meta-gesture. `DEVELOPMENT.md` is for people who
build or change it. The root `CLAUDE.md` describes what goes where.

## Keep logic out of `src/win/` and `macos/`

`config.rs`, `gesture.rs`, `keys.rs`, `vk.rs`, `mac.rs`, `menu.rs`, `editor.rs`, `ffi.rs`, `art.rs`
and `icon_res.rs` do not touch the operating system, so they build and test on any platform (`ffi.rs`
with the `ffi` feature). All but `icon_res.rs` make up the library (`src/lib.rs`); `main.rs` and
`src/win/` are the Windows binary built on it, and the macOS app in `macos/` calls it through the
bindings `ffi.rs` declares. Anything that decides behaviour — which gesture the pointer drew, what
a gesture comes to over an application, what a config accepts, what the settings window edits and
checks before saving, which menu items a search finds — belongs there, with its tests. `src/win/`
and `macos/` pass values between the library and the operating system and should stay thin: the
macOS app reads an application's menu bar through the accessibility API (`Menus.swift`) and hands
the items to `ffi.rs` as they are reported.

Keys and modifiers are `keys::Key` and `keys::Modifiers`, never an operating system's codes:
`vk.rs` is the one place that converts between them and Windows' virtual-key codes, and `mac.rs`
the one for macOS's key codes and event flags.

The Swift bindings and the C header in `macos/` are written by `macos/build.sh` from the built
library and are not committed. Change `ffi.rs`, never the generated files. A record or object
exported there must not take a name the Swift standard library already has (`CommandLine`, say):
the generated type would hide it in the app's module.

## The hook must stay quick

`src/win/mousehook.rs` runs inside a low-level mouse hook, which Windows drops when it answers too
slowly. It only follows the pointer and posts `WM_APP_GESTURE`; looking up the gesture, bringing
the window forward and sending keys happen in the main window's procedure. The macOS event tap
(`MouseTap.swift`) works the same way: `AppDelegate` acts on a gesture in a task after the tap has
answered.

## The state borrow rule

`with_app` (`src/win/app.rs`) lends the resident part's state out of a `RefCell`. Inside its
closure, never call anything that sends a message to one of meta-gesture's own windows, such as a
message box. The window procedure re-enters, the borrow fails, and the work is silently dropped.
Copy what you need out of the closure and make the call after it.

The settings window (`src/win/settings_ui.rs`) is a `windows-reactor` component running in its
own process; its state lives in the component and changes only in `update`.

## Keep explanations out of the UI

Label controls; do not add sentences that explain what a control does or how to use it. A line of
text in the settings window earns its place only when it tells the user something they could not
see otherwise, such as why an action failed.

## Keeping the tests in step

`DEVELOPMENT.md` describes what the tests cover and what they cannot reach. When meta-gesture's
behaviour changes, the file that covers it changes in the same commit:

| Changed | Follow it in |
|---|---|
| Which strokes the pointer's path makes, or how a gesture is written | `mod tests` in `src/gesture.rs` |
| The config file's fields or validation, or what a gesture comes to over an application | `mod tests` in `src/config.rs`, and the example in `DEVELOPMENT.md` |
| Key names, how combinations are written, or how keys are recorded | `mod tests` in `src/keys.rs` |
| Which virtual-key code a key or modifier is, or which keys are extended | `mod tests` in `src/vk.rs` |
| Which macOS key code or event flag a key or modifier is, or which keys a menu item's shortcut is | `mod tests` in `src/mac.rs` |
| How a menu item's path is shown, or which items a search finds and in what order | `mod tests` in `src/menu.rs` |
| What the settings window edits, lists or checks | `mod tests` in `src/editor.rs` |
| A preset's commands, its applications or their keys | `mod tests` in `src/preset.rs`, and the sources at its top: keys come only from an application's own documentation, or from its maker's source code when that documentation cannot be read |
| What the macOS app is told to do with a press, or what its settings window shows and saves | `mod tests` in `src/ffi.rs` |
| The icon's picture, or how it is written into the executable | `mod tests` in `src/art.rs` or `src/icon_res.rs` |
| The main window's class name, the config folder, the `Run` value name, or a second start opening the settings | `installer/installer.nsi`, which closes meta-gesture by that class, removes those on uninstall, and opens the settings from its finish page by starting meta-gesture twice |

Everything under `src/win/` and `macos/` is out of reach of `cargo test`; `DEVELOPMENT.md` lists
the manual checks. Do not write a case that pretends to cover it.
