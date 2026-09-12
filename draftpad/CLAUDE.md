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
