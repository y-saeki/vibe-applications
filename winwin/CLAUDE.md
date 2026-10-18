# winwin

Repository-wide conventions are in the root `CLAUDE.md`; these apply on top of them.

## Documentation layout

`README.md` is for people who install and use winwin. `DEVELOPMENT.md` is for people who
build or change it. The root `CLAUDE.md` describes what goes where.

## Keep logic out of `src/win/` and `macos/`

`config.rs`, `layout.rs`, `hotkey.rs`, `vk.rs`, `mac.rs`, `ffi.rs` (with `ffi/settings.rs`),
`cycle.rs`, `restore.rs`, `draft.rs`, `art.rs` and `icon_res.rs` do not touch the operating
system, so they build and test on any platform (`ffi.rs` with the `ffi` feature). All but
`icon_res.rs` make up the library (`src/lib.rs`); `main.rs` and `src/win/` are the Windows
binary built on it, and the macOS app in `macos/` calls it through the bindings `ffi.rs`
declares. Anything that decides behaviour —
what a placement resolves to, what a config accepts, what the settings window checks before
saving, which key presses a shortcut takes — belongs there, with its tests. `src/win/` and
`macos/` pass values between the library and the operating system and should stay thin.

Keys and modifiers are `hotkey::Key` and `hotkey::Modifiers`, never an operating system's
codes: `vk.rs` is the one place that converts between them and Windows' virtual-key codes or
`MOD_*` flags, and `mac.rs` the one for macOS's key codes and event flags.

The Swift bindings and the C header in `macos/` are written by `macos/build.sh` from the built
library and are not committed. Change `ffi.rs`, never the generated files.

## The state borrow rule

`with_app` (`src/win/app.rs`) lends the resident part's state out of a `RefCell`. Inside
its closure, never call anything that sends a message to one of winwin's own windows, such
as a message box. The window procedure re-enters, the borrow fails, and the work is
silently dropped. Copy what you need out of the closure and make the call after it.

The settings window (`src/win/settings_ui.rs`) is a `windows-reactor` component running in
its own process; its state lives in the component and changes only in `update`.

## Keep explanations out of the UI

Label controls; do not add sentences that explain what a control does or how to use it. A
line of text in the settings window earns its place only when it tells the user something
they could not see otherwise, such as why an action failed.

## Keeping the tests in step

`DEVELOPMENT.md` describes what the tests cover and what they cannot reach. When winwin's
behaviour changes, the file that covers it changes in the same commit:

| Changed | Follow it in |
|---|---|
| How a placement turns into a rectangle, or which screen a window is on | `mod tests` in `src/layout.rs` |
| The config file's fields, defaults or validation | `mod tests` in `src/config.rs`, and the example in `DEVELOPMENT.md` |
| Key names or which combinations are allowed | `mod tests` in `src/hotkey.rs` |
| Which virtual-key code or `MOD_*` flag a key or modifier is | `mod tests` in `src/vk.rs` |
| Which macOS key code or event flag a key or modifier is, or how AppKit's frames are turned | `mod tests` in `src/mac.rs` |
| Which key presses the macOS app takes, and what it is told to do with them, including resets | `mod tests` in `src/ffi.rs` |
| What the macOS settings window shows, records or saves | `mod tests` in `src/ffi/settings.rs` |
| How entries that share a shortcut are grouped and take turns, or how a shortcut of several strokes waits for the next | `mod tests` in `src/cycle.rs` |
| Which size a dragged window gets back, where it lands, and how it is kept on the window | `mod tests` in `src/restore.rs` |
| What the settings window stores, checks on save, or shows in its list | `mod tests` in `src/draft.rs` |
| The icon's picture, or how it is written into the executable | `mod tests` in `src/art.rs` or `src/icon_res.rs` |
| The main window's class name, the config folder, the `Run` value name, or a second start opening the settings | `installer/installer.nsi`, which closes winwin by that class, removes those on uninstall, and opens the settings from its finish page by starting winwin twice |

Everything under `src/win/` and `macos/` is out of reach of `cargo test`; `DEVELOPMENT.md`
lists the manual checks. Do not write a case that pretends to cover it.
