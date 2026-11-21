# pecha-player

Repository-wide conventions are in the root `CLAUDE.md`; these apply on top of them.

## Documentation layout

`README.md` is for people who install and use pecha-player. `DEVELOPMENT.md` is for people
who build or change it, including how decks are loaded and what keeps an untrusted PDF or URL
contained.

## Untrusted input stays contained

Every deck comes from somewhere the app does not control. Changes that touch loading keep
to what `DEVELOPMENT.md` describes under 仕組み:

- PDF.js only draws: no document scripting, no XFA, no annotation layer, nothing but a
  canvas in the DOM.
- A pasted URL contributes an id and nothing else; requests are rebuilt from it, and
  redirects are checked hop by hop against the allowed hosts.
- Windows that show remote pages get no capability.

## Keeping the tests in step

`DEVELOPMENT.md` describes the two suites. When behaviour changes, the file that covers it
changes in the same commit:

| Changed | Follow it in |
|---|---|
| Loading a deck, its messages and the sign-in flow (`src/main.ts`, `src/deck.ts`) | `tests/e2e/loading.spec.ts` |
| The run: timing, counter, clock, shutter, keys, display options (`src/timeline.ts`, `src/main.ts`) | `tests/e2e/playback.spec.ts` |
| A command the frontend invokes | `handle` in `tests/e2e/harness/backend.ts`. It throws on a command it does not know |
| URL parsing, redirect checks, PDF detection (`src-tauri/src/google.rs`) | `mod tests` in the same file |
| An asset the page loads, or the `csp` in `src-tauri/tauri.conf.json` | `tests/e2e/csp.spec.ts`, the only spec served under that policy |

What the suites cannot reach is listed in `DEVELOPMENT.md`; assert the `invoke` that was made
and leave the rest to the manual pass.
