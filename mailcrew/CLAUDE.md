# mailcrew

Repository-wide conventions are in the root `CLAUDE.md`; these apply on top of them.

## The name

The application is called Mail Crew in everything a user reads: the window, the bundle
(`PRODUCT_NAME` and `CFBundleDisplayName` in `project.yml`), `README.md`, release titles, strings
in the UI. `mailcrew` stays where it is an identifier: the directory, the crate, the bundle
identifier (`com.ysaeki.mailcrew`), the credential store's service name, the release tag and the
workflow files. Changing the bundle identifier or the service name would strand every stored
sign-in, so neither follows the display name.

## What lives where

Mail Crew is a native macOS app (SwiftUI) over one Rust crate. It replaced the Tauri app in 0.8,
which also ended the Windows build. `core/` is the crate: Gmail, sign-in and the stored
accounts, message bodies and their sanitising, attachments, the sender checks, and the settings
files, behind `Core` (`core/src/service.rs`). The app calls `Core` through the UniFFI bindings in
`core/src/ffi.rs`, which `core/swift/` wraps as a Swift package, and does itself only what the OS
does for it (opening the browser or a file, finding the downloads folder). Nothing in `core/`
depends on the UI. Keep the Windows code in `core/` (such as the Credential Manager layout in
`credentials.rs`) building: it is there for a Windows app to come.

## The message view is a security boundary

A message's HTML is untrusted, and two layers keep it from doing anything:
`core/src/sanitize.rs` strips what could run, and `MailCrew/MessageFrame.swift` and
`MessageWebView.swift` show the rest in a WKWebView under locks of its own. `DEVELOPMENT.md`
explains both. Changing either keeps these true:

- `allowsContentJavaScript` stays off. The script that measures the body is the app's own and
  runs in a content world of its own.
- Nothing from the network loads in a message until the user presses 表示する for it, or has
  chosen この差出人は常に表示 for its sender (`core/src/image_senders.rs`). The choice is
  kept per address, as Gmail keeps it, never per domain. The document's CSP and the
  `WKContentRuleList` each block every other load on their own; keep both.
- The web view never scrolls; the card body does, and nothing in a card overflows sideways,
  so a horizontal swipe always moves the strip.
- Only the app's own `about:blank` document may load. A clicked link never navigates the view: it
  leaves only through `isOpenableLink` (`core/src/links.rs`), which allows http, https and mailto.
  No new windows. Link previews stay off, since they load the link.
- Images the message carries inside itself reach the view only as `data:` URLs that
  `sanitize.rs` puts in place of their `cid:` URLs, so the document's `img-src data:` is enough
  and nothing about them goes to the network.

## Attachments are opened by kind, not trusted

What a file does when opened is decided by its extension, and the sender picks the extension.
So only the kinds listed in `OPENABLE` in `core/src/attachments.rs` — documents, images,
audio, video — are ever handed to the OS to open; everything else can only be saved. The check
is made in Rust on the name as it will be written (`safe_file_name`), not on what the app says,
and the app shows no 開く for the rest. Do not add a kind that runs code or opens a browser or
installer: programs, scripts, shortcuts, archives, HTML, SVG, macro-enabled Office files, disk
images. Saving never overwrites a file that is already there.

## Sender checks and BIMI

The row under a card's subject shows Gmail's SPF, DKIM and DMARC results and Mail Crew's own BIMI
and mark certificate checks. `DEVELOPMENT.md` has the steps. Changing them keeps these true:

- Only Gmail's `Authentication-Results` is read: the topmost one, and only when its authserv-id
  is Gmail's (`GMAIL_AUTHSERV_ID` in `core/src/authres.rs`). A sender can write the header
  too.
- Nothing about BIMI is looked up unless Gmail's DMARC result is `pass`, and nothing at all
  while the user has turned BIMI off in the settings (`preferences.json`, on by default).
  Lookups go out from Rust; the app only ever gets a `data:` URL. It draws that URL in a
  WKWebView under the message's locks (no JavaScript, the blocking `WKContentRuleList`, a CSP of
  `img-src data:`), and only when it starts with `data:image/svg+xml`.
- A logo is shown only from a mark certificate that chains to a root in `core/bimi-roots/`.
  The record's `l=` file is never fetched.

## Every action names its account

A card carries the address of the account its thread is in (`Card.account`), and every command
that reads or changes mail takes that address. Thread, message and label ids are only unique
within one account, so nothing in the app is keyed by an id alone. On macOS the stored sign-ins
are `accounts.json` in the config folder, mode 0600, not the Keychain: the app is not signed
with a certificate Apple issued, so the Keychain asked for the password after every update.
Do not move them back into the Keychain until the app is signed with a Developer ID. Up to
0.8.0 they were the Keychain entry `all-accounts`, and before 0.4 the Windows layout; whatever
is in the Keychain is read once, written to the file and deleted. On Windows, whose entries are
capped at 2,560 bytes, they are one Credential Manager entry per account plus an `accounts`
index, and `google-account`, the pre-0.2 single account, is read once and moved.

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

`DEVELOPMENT.md` describes the suites. When mailcrew's behaviour changes, the file that
covers it changes in the same commit:

| Changed | Follow it in |
|---|---|
| A field on `Card` or `MessageContent` (`core/src/card.rs`) | `mod tests` in `card.rs`, the record in `ffi.rs`, and `FakeCore` |
| How accounts are stored, or the inbox of several accounts is merged | `mod tests` in `core/src/credentials.rs` and `core/src/accounts.rs` |
| How the senders whose images are always shown are stored | `mod tests` in `core/src/image_senders.rs` |
| What the sanitiser removes or keeps, or how embedded images are put in | `mod tests` in `core/src/sanitize.rs` |
| Which parts are attachments or embedded images | `mod tests` in `core/src/mime.rs` and `card.rs` |
| How attachments are named and written, or which kinds open | `mod tests` in `core/src/attachments.rs` |
| Which links may be opened outside the app | `mod tests` in `core/src/links.rs` |
| How Gmail's `Authentication-Results` are read | `mod tests` in `core/src/authres.rs` |
| The DMARC, BIMI record and mark certificate checks | `mod tests` in `core/src/bimi.rs` |
| How preferences are stored | `mod tests` in `core/src/preferences.rs` |
| A function, object or record the app calls (`core/src/ffi.rs`) | `mod tests` in `ffi.rs`, and `core/swift/Tests/` for what crosses into Swift |
| What the app shows at startup, after a sign-in or sign-out, or how it loads the inbox (`MailCrew/AppModel.swift`) | `MailCrewTests/AppModelTests.swift`, and `FakeCore` for a call it newly makes |
| When the app fetches a body, which images it lets in, or which links it opens (`AppModel.swift`) | `MailCrewTests/MessageBodyTests.swift`, and `FakeCore` for a call it newly makes |
| What a card's 完了, タグ, あとで読む or スター asks of the Rust side, how the card leaves the strip or comes back, or the tag picker's rows (`AppModel.swift`) | `MailCrewTests/CardActionsTests.swift`, and `FakeCore` for a call it newly makes |
| The app's checks under the subject, when it looks up BIMI, which logo it shows or the logo's document (`AppModel.swift`, `TrustRow.swift`), or what a card's attachment button lists, opens and saves | `MailCrewTests/TrustAndAttachmentsTests.swift`, and `FakeCore` for a call it newly makes |
| The app's sidebar and its tabs, what each tab fetches, まとめて完了 or the end of the strip, or what the settings window asks of the Rust side (`AppModel.swift`, `InboxView.swift`, `SettingsView.swift`) | `MailCrewTests/SidebarAndSettingsTests.swift`, and `FakeCore` for a call it newly makes |
| The app's pull to reload: which gestures reload, or how it reads the scroll view (`PullToRefresh.swift`) | `MailCrewTests/PullGestureTests.swift` |
| Where a swipe along the strip comes to rest (`StripScroll.swift`) | `MailCrewTests/StripScrollTests.swift` |
| The app's message document, content rules or navigation policy (`MessageFrame.swift`), or how `MessageWebView.swift` loads, measures and scales a body | `MailCrewTests/MessageFrameTests.swift` and `MessageWebViewTests.swift` |

`DEVELOPMENT.md` lists what these tests cannot reach — Google itself, the credential store,
the browser a link opens in. Assert the call that reached `FakeCore` and leave the rest to the
manual pass; do not write a case that looks like it covers one of them.
