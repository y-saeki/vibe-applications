# draftpad

Repository-wide conventions are in the root `CLAUDE.md`; these apply on top of them.

## How draftpad is described to users

draftpad's pitch is an editor with no files and no saving: text is simply there again the
next time the window opens. User-facing text — `README.md`, the release notes, strings in
the UI — therefore avoids the words 保存 and ファイル for that behaviour, even though the
implementation does write a `state.json`. Say the text carries over (引き継がれる), not that
it is saved.

That file, and how it is written, is a developer-facing detail: it belongs in
`DEVELOPMENT.md`.

## Documentation layout

`README.md` is for people who install and use draftpad. `DEVELOPMENT.md` is for people who
build or change it. The root `CLAUDE.md` describes what goes where.

## Keeping the tests in step

`DEVELOPMENT.md` describes the two suites. When draftpad's behaviour changes, the file that
covers it changes in the same commit:

| Changed | Follow it in |
|---|---|
| An entry or shortcut in the command table (`src/commands.ts`) | `tests/e2e/shortcuts.spec.ts` (the Windows path) **and** `tests/e2e/menu.spec.ts` (the macOS path) |
| A preferences control | `tests/e2e/preferences.spec.ts` |
| What is restored at startup | `tests/e2e/startup.spec.ts` |
| Editing behaviour, or the status bar | `tests/e2e/editing.spec.ts` |
| A field on `State` | `DEFAULT_STATE` in `tests/e2e/harness/backend.ts`, which mirrors `impl Default for State`, and `mod tests` in `src-tauri/src/state.rs` |
| A command the frontend invokes | `handle` in `tests/e2e/harness/backend.ts`. It throws on a command it does not know, so the tests fail until the command is added |

Both platform paths resolve against the same command table, so a new entry there needs a case
in each of those two specs, not one.

Removing a feature removes its cases. A test still passing for behaviour draftpad no longer has
is the failure this list exists to prevent.

Two things when writing a new case:

- Put text into the editor through the `typeInEditor` helper, never `page.keyboard.type`.
  Typing character by character races with CodeMirror in WebKit; the helper says why.
- `DEVELOPMENT.md` lists what these tests cannot reach — native menus, the jump list, the IME,
  what the window actually does. Assert the `invoke` that was made and leave the rest to the
  manual pass; do not write a case that looks like it covers one of them.
