# mailcrew

Repository-wide conventions are in the root `CLAUDE.md`; these apply on top of them.

## The name

The application is called Mail Crew in everything a user reads: the window, the bundle
(`productName`), `README.md`, release titles, strings in the UI. `mailcrew` stays where it is
an identifier: the directory, the package names, `identifier`, the credential store's service
name, the release tag and the workflow files. Changing `identifier` or the service name would
strand every stored sign-in, so neither follows the display name.

## The message frame is a security boundary

A message's HTML is untrusted, and two layers keep it from doing anything:
`src-tauri/src/sanitize.rs` strips what could run, and `src/message-frame.ts` shows the
rest in an iframe sandboxed with `allow-same-origin` alone and a Content-Security-Policy of
its own. `DEVELOPMENT.md` explains both. Changing either keeps these true:

- The frame never gets `allow-scripts`. With `allow-same-origin` beside it, the sandbox
  would no longer hold.
- Nothing from the network loads in a message until the user presses 表示する for it, or has
  chosen この差出人は常に表示 for its sender (`src-tauri/src/image_senders.rs`). The choice is
  kept per address, as Gmail keeps it, never per domain.
- The frame never scrolls; the card body does, and nothing in a card overflows sideways,
  so a horizontal swipe always moves the strip. `tests/e2e/message.spec.ts` holds both.
- Links never navigate the frame. WebView2 hands them to `open_link`; in WKWebView, where the
  page's listeners do not run inside the frame, `src-tauri/src/links.rs` turns the navigation
  away and opens the link outside. Both open http, https and mailto only.
- The webview is granted no opener permission. Only Rust opens URLs.
- WebKit checks the app's `frame-src` before it asks `links.rs` about a frame's navigation, and
  drops what the policy refuses without a word. `src-tauri/tauri.macos.conf.json` therefore lets
  `http:`, `https:` and `mailto:` through on macOS, where the guard turns every one of them away.
  Windows keeps `'self'`: WebView2 does not ask about frames, so the policy is all that stops one
  there.
- Images the message carries inside itself reach the frame only as `data:` URLs that
  `sanitize.rs` puts in place of their `cid:` URLs, so the frame's `img-src data:` is enough
  and nothing about them goes to the network from the page.

`dangerousDisableAssetCspModification: ["style-src"]` in `tauri.conf.json` is deliberate:
Tauri's style nonce would switch off `'unsafe-inline'`, and every message's inline styles
with it. Do not remove it without replacing what it does.

## Attachments are opened by kind, not trusted

What a file does when opened is decided by its extension, and the sender picks the extension.
So only the kinds listed in `OPENABLE` in `src-tauri/src/attachments.rs` — documents, images,
audio, video — are ever handed to the OS to open; everything else can only be saved. The check
is made in Rust on the name as it will be written (`safe_file_name`), not on what the page says,
and the page shows no 開く for the rest. Do not add a kind that runs code or opens a browser or
installer: programs, scripts, shortcuts, archives, HTML, SVG, macro-enabled Office files, disk
images. Saving never overwrites a file that is already there.

## Sender checks and BIMI

The row under a card's subject shows Gmail's SPF, DKIM and DMARC results and Mail Crew's own BIMI
and mark certificate checks. `DEVELOPMENT.md` has the steps. Changing them keeps these true:

- Only Gmail's `Authentication-Results` is read: the topmost one, and only when its authserv-id
  is Gmail's (`GMAIL_AUTHSERV_ID` in `src-tauri/src/authres.rs`). A sender can write the header
  too.
- Nothing about BIMI is looked up unless Gmail's DMARC result is `pass`, and nothing at all
  while the user has turned BIMI off in the settings (`preferences.json`, on by default).
  Lookups go out from Rust; the page only ever gets a `data:` URL.
- A logo is shown only from a mark certificate that chains to a root in `src-tauri/bimi-roots/`.
  The record's `l=` file is never fetched.

## Every action names its account

A card carries the address of the account its thread is in (`Card.account`), and every command
that reads or changes mail takes that address. Thread, message and label ids are only unique
within one account, so nothing on the page is keyed by an id alone. On macOS the stored sign-ins
are one credential entry, `all-accounts`, since the Keychain asks once per entry after every
update; on Windows, whose entries are capped at 2,560 bytes, they are one entry per account plus
an `accounts` index. Both older layouts (that index on macOS before 0.4, and `google-account`,
the pre-0.2 single account) are read once and moved.

## Mind the Gmail quota

Each user gets 6,000 quota units a minute, and a full thread costs 120 of them, so a page of
50 full threads is already over. The strip is therefore built from `format=metadata` threads,
and a body is fetched with `get_message` only when its card comes near the screen. A new call
that reads mail states its cost against the usage limits page before it goes in.

## Labels change on message ids, not threads

Every action sends the message ids the card was built from (`messages.batchModify`), so a
reply that arrives while a card is on screen stays in the inbox. Keep new actions on that
path rather than `threads.modify`.

## Keeping the tests in step

`DEVELOPMENT.md` describes the two suites. When mailcrew's behaviour changes, the file that
covers it changes in the same commit:

| Changed | Follow it in |
|---|---|
| Sign-in, sign-out, or which screen shows at startup | `tests/e2e/sign-in.spec.ts` |
| Adding or removing an account, a sender whose images are always shown, or the BIMI setting, in the settings | `tests/e2e/settings.spec.ts` |
| The sidebar and the views in it, the toolbar's title, or making, editing and deleting a view | `tests/e2e/views.spec.ts` |
| How cards are laid out, the end of the strip, the empty state, reloading | `tests/e2e/inbox.spec.ts` |
| A toolbar button or まとめて完了 | `tests/e2e/actions.spec.ts` |
| The tag picker | `tests/e2e/tags.spec.ts` |
| The list of a card's attachments, opening and saving them | `tests/e2e/attachments.spec.ts` |
| How a body is shown: the frame, links, images, scaling, lazy mounting, the checks under the subject, the BIMI logo | `tests/e2e/message.spec.ts` |
| The `csp` in `src-tauri/tauri.conf.json` or `tauri.macos.conf.json`, or anything the page loads from elsewhere | `tests/e2e/csp.spec.ts` |
| A field on `Card` or `MessageContent` (`src-tauri/src/card.rs`) | `src/backend.ts`, `card()` in `tests/e2e/fixtures.ts`, and `mod tests` in `card.rs` |
| A command the frontend invokes | `handle` in `tests/e2e/harness/backend.ts`. It throws on a command it does not know |
| How accounts are stored, or the inbox of several accounts is merged | `mod tests` in `src-tauri/src/credentials.rs` and `src-tauri/src/accounts.rs` |
| How the senders whose images are always shown are stored | `mod tests` in `src-tauri/src/image_senders.rs` |
| What the sanitiser removes or keeps, or how embedded images are put in | `mod tests` in `src-tauri/src/sanitize.rs` |
| Which parts are attachments or embedded images | `mod tests` in `src-tauri/src/mime.rs` and `card.rs` |
| How attachments are named and written, or which kinds open | `mod tests` in `src-tauri/src/attachments.rs` |
| Which navigations the webview allows | `mod tests` in `src-tauri/src/links.rs` |
| How Gmail's `Authentication-Results` are read | `mod tests` in `src-tauri/src/authres.rs` |
| The DMARC, BIMI record and mark certificate checks | `mod tests` in `src-tauri/src/bimi.rs` |
| How preferences are stored | `mod tests` in `src-tauri/src/preferences.rs` |

`DEVELOPMENT.md` lists what these tests cannot reach — Google itself, the credential store,
the browser a link opens in. Assert the `invoke` that was made and leave the rest to the
manual pass; do not write a case that looks like it covers one of them.
