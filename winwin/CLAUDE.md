# winwin

Repository-wide conventions are in the root `CLAUDE.md`; these apply on top of them.

## Documentation layout

`README.md` is for people who install and use winwin. `DEVELOPMENT.md` is for people who
build or change it. The root `CLAUDE.md` describes what goes where.

## Keep logic out of `src/win/`

`config.rs`, `layout.rs`, `hotkey.rs`, `cycle.rs`, `draft.rs`, `art.rs` and `icon_res.rs` do not touch Win32, so they build and
test on any platform. Anything that decides behaviour — what a placement resolves to, what a
config accepts, what the settings window checks before saving — belongs there, with its
tests. `src/win/` passes values between those modules and Win32 and should stay thin.

## The state borrow rule

`with_app` (`src/win/app.rs`) lends the resident part's state out of a `RefCell`. Inside
its closure, never call anything that sends a message to one of winwin's own windows, such
as a message box. The window procedure re-enters, the borrow fails, and the work is
silently dropped. Copy what you need out of the closure and make the call after it.

The settings window (`src/win/settings_ui.rs`) is a `windows-reactor` component running in
its own process; its state lives in the component and changes only in `update`.

## Keeping the tests in step

`DEVELOPMENT.md` describes what the tests cover and what they cannot reach. When winwin's
behaviour changes, the file that covers it changes in the same commit:

| Changed | Follow it in |
|---|---|
| How a placement turns into a rectangle | `mod tests` in `src/layout.rs` |
| The config file's fields, defaults or validation | `mod tests` in `src/config.rs`, and the example in `DEVELOPMENT.md` |
| Key names or which combinations are allowed | `mod tests` in `src/hotkey.rs` |
| How entries that share a shortcut are grouped and take turns | `mod tests` in `src/cycle.rs` |
| What the settings window stores, checks on save, or shows in its list | `mod tests` in `src/draft.rs` |
| The icon's picture, or how it is written into the executable | `mod tests` in `src/art.rs` or `src/icon_res.rs` |
| The main window's class name, the config folder, or the `Run` value name | `installer/installer.nsi`, which closes winwin by that class and removes those on uninstall |

Everything under `src/win/` is out of reach of `cargo test`; `DEVELOPMENT.md` lists the
manual checks. Do not write a case that pretends to cover it.
