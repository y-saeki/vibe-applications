# diffpad

Repository-wide conventions are in the root `CLAUDE.md`; these apply on top of them.

## How diffpad is described to users

diffpad's pitch is a comparison pad with no files and no saving: the two texts are simply
there again the next time the window opens. User-facing text — `README.md`, the release
notes, strings in the UI — therefore avoids the words 保存 and ファイル for that behaviour,
even though the implementation does write a `state.json`. Say the text carries over
(引き継がれる), not that it is saved.

That file, and how it is written, is a developer-facing detail: it belongs in
`DEVELOPMENT.md`.

The two panes are 左 / 右 or 比較元 / 比較先 to users; `a` / `b` is what the code and the
merge view call them, and stays in the code.

## Documentation layout

`README.md` is for people who install and use diffpad. `DEVELOPMENT.md` is for people who
build or change it. The root `CLAUDE.md` describes what goes where.

## Chrome is built from the tokens, not from numbers

Every size, spacing and color in `src/style.css` comes from a custom property in its
`:root` blocks, and `pnpm lint:style` fails the build when a rule writes a number or a
color of its own. Adding a control therefore means reaching for a token — and when none
fits, adding one to `:root` and raising that family's budget in `lint-style.mjs` in the
same commit, rather than reaching for a literal.

Before adding a token, try the existing ones: a value one or two pixels away from a token
that already exists almost always wants to be that token. `DEVELOPMENT.md` has the
families, the value rules and the three judgements the check cannot make.

## Keeping the tests in step

`DEVELOPMENT.md` describes the two suites. When diffpad's behaviour changes, the file that
covers it changes in the same commit:

| Changed | Follow it in |
|---|---|
| An entry or shortcut in the command table (`src/commands.ts`) | `tests/e2e/shortcuts.spec.ts` (the Windows path) **and** `tests/e2e/menu.spec.ts` (the macOS path) |
| A preferences control | `tests/e2e/preferences.spec.ts` |
| What is restored at startup | `tests/e2e/startup.spec.ts` |
| Editing, the diff, its marks, or the status bar | `tests/e2e/diff.spec.ts` |
| A field on `State` | `DEFAULT_STATE` in `tests/e2e/harness/backend.ts`, which mirrors `impl Default for State`, and `mod tests` in `src-tauri/src/state.rs` |
| A command the frontend invokes | `handle` in `tests/e2e/harness/backend.ts`. It throws on a command it does not know, so the tests fail until the command is added |
| An asset the page loads from anywhere but its own files (a `data:` URI, say), or the `csp` in `src-tauri/tauri.conf.json` | `tests/e2e/csp.spec.ts`, which is the only spec served under that policy |

Both platform paths resolve against the same command table, so a new entry there needs a case
in each of those two specs, not one.

Removing a feature removes its cases. A test still passing for behaviour diffpad no longer has
is the failure this list exists to prevent.

Two things when writing a new case:

- Put text into a pane through the `typeInEditor` helper, never `page.keyboard.type`.
  Typing character by character races with CodeMirror in WebKit; the helper says why.
- `DEVELOPMENT.md` lists what these tests cannot reach — native menus, the IME, what the
  window actually does. Assert the `invoke` that was made and leave the rest to the manual
  pass; do not write a case that looks like it covers one of them.
