# winwin

Repository-wide conventions are in the root `CLAUDE.md`; these apply on top of them.

## Documentation layout

`README.md` is for people who install and use winwin. `DEVELOPMENT.md` is for people who
build or change it. The root `CLAUDE.md` describes what goes where.

## Keep logic out of `src/win/`

`config.rs`, `layout.rs`, `hotkey.rs` and `draft.rs` do not touch Win32, so they build and
test on any platform. Anything that decides behaviour — what a placement resolves to, what a
config accepts, what the settings window checks before saving — belongs there, with its
tests. `src/win/` passes values between those modules and Win32 and should stay thin.

## The state borrow rule

`with_app` (`src/win/app.rs`) and `with_state` (`src/win/settings.rs`) lend the window
state out of a `RefCell`. Inside their closures, never call anything that sends a message
to one of winwin's own windows: a message box, `SetWindowTextW` on a control,
`SetWindowPos` on the settings window. The window procedure re-enters, the borrow fails,
and the work is silently dropped. Copy what you need out of the closure and make the call
after it.

## Keeping the tests in step

`DEVELOPMENT.md` describes what the tests cover and what they cannot reach. When winwin's
behaviour changes, the file that covers it changes in the same commit:

| Changed | Follow it in |
|---|---|
| How a placement turns into a rectangle | `mod tests` in `src/layout.rs` |
| The config file's fields, defaults or validation | `mod tests` in `src/config.rs`, and the example in `DEVELOPMENT.md` |
| Key names or which combinations are allowed | `mod tests` in `src/hotkey.rs` |
| What the settings window stores, checks on save, or shows in its list | `mod tests` in `src/draft.rs` |
| The main window's class name, the config folder, or the `Run` value name | `installer/installer.nsi`, which closes winwin by that class and removes those on uninstall |

Everything under `src/win/` is out of reach of `cargo test`; `DEVELOPMENT.md` lists the
manual checks. Do not write a case that pretends to cover it.
