# myaku

Repository-wide conventions are in the root `CLAUDE.md`; these apply on top of them.

## Documentation layout

`README.md` is for people who install and use myaku. `DEVELOPMENT.md` is for people who
build or change it, including why the stack is what it is. The root `CLAUDE.md` describes
what goes where.

## Keep logic out of `src/win/` and `macos/`

Everything in `src/` except `main.rs` and `src/win/` makes up the library (`src/lib.rs`),
which does not touch the operating system and builds and tests on any platform (`ffi.rs`
with the `ffi` feature). What the list shows and in what order, how a value is written or
how tall its graph is, how `ps`, `nettop` or `SystemProcessInformation` output is read —
all of that belongs there, with its tests. `src/win/` and `macos/` pass values between the
library and the operating system and draw what it returns; they should stay thin.

The Swift bindings and the C header in `macos/` are written by `macos/build.sh` from the
built library and are not committed. Change `ffi.rs`, never the generated files.

## The state borrow rules

`with_app` (`src/win/app.rs`) and `with_flyout` (`src/win/flyout.rs`) lend window state out
of a `RefCell`. Inside their closures, never call anything that sends a message to one of
myaku's own windows (`SetWindowPos`, `ShowWindow`, a message box): the window procedure
re-enters, the borrow fails, and the work is silently dropped. Copy what you need out of the
closure and make the call after it.

## Keep explanations out of the UI

Label controls; do not add sentences that explain what a control does.

## Keeping the tests in step

`DEVELOPMENT.md` lists what the tests cover and the manual checks for what they cannot
reach. When myaku's behaviour changes, the `mod tests` of the file that decides it changes
in the same commit. Everything under `src/win/` and `macos/` is out of reach of
`cargo test`; do not write a case that pretends to cover it.
