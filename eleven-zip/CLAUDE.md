# eleven-zip

Repository-wide conventions are in the root `CLAUDE.md`; these apply on top of them.

## Documentation layout

`README.md` is for people who install and use eleven-zip. `DEVELOPMENT.md` is for people who
build or change it. The root `CLAUDE.md` describes what goes where.

## Keep logic out of `src/win/`

`cli.rs`, `config.rs`, `archive.rs`, `release.rs`, `store.rs`, `shell.rs`, `art.rs` and
`icon_res.rs` do not touch Win32, the network or 7-Zip, so they build and test on any platform.
Anything that decides behaviour — where an archive is extracted to, which release is installed
and whether a download is it, what is written to the registry and what is removed again —
belongs there, with its tests. `src/win/` runs processes, makes requests and calls Win32 with
what those modules decide, and should stay thin.

## The registry is undone from the same plan

`shell::register` and `shell::unregister` describe everything eleven-zip writes under
HKEY_CURRENT_USER. A new key or value goes into both, and the test that checks unregistering
removes everything registering writes keeps them in step. Never remove a key other
applications share (an extension's key, `OpenWithProgids`, `RegisteredApplications`): remove
eleven-zip's value from it.

## Keep explanations out of the UI

Label controls; do not add sentences that explain what a control does or how to use it. A
line of text in the settings window earns its place only when it tells the user something
they could not see otherwise, such as why an update failed.

## Keeping the tests in step

`DEVELOPMENT.md` describes what the tests cover and what they cannot reach. When eleven-zip's
behaviour changes, the file that covers it changes in the same commit:

| Changed | Follow it in |
|---|---|
| The command line, or what the registry's commands pass | `mod tests` in `src/cli.rs` |
| The config file's fields or defaults | `mod tests` in `src/config.rs`, and the example in `DEVELOPMENT.md` |
| Where an archive goes, its folder's name, how 7-Zip's listing or exit codes are read | `mod tests` in `src/archive.rs` |
| Which release and file are installed, the digest check, how often updates are checked | `mod tests` in `src/release.rs` |
| How versions of 7-Zip are kept, switched and removed | `mod tests` in `src/store.rs` |
| What is written to or removed from the registry, or the extensions handled | `mod tests` in `src/shell.rs` |
| The icon's picture, or how it is written into the executable | `mod tests` in `src/art.rs` or `src/icon_res.rs` |
| The settings window's title, the config folder or the 7-Zip folder | `installer/installer.nsi`, which closes the window by that title and removes those on uninstall |

Everything under `src/win/` is out of reach of `cargo test`; `DEVELOPMENT.md` lists the
manual checks. Do not write a case that pretends to cover it.
