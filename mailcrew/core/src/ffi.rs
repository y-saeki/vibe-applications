//! The library as the native macOS app calls it, through the Swift bindings
//! UniFFI writes from this file. `MailCrew` is `Core` with its own Tokio
//! runtime: Swift awaits the methods from any thread, and the work runs on
//! that runtime, where reqwest, hickory and the sign-in's listener expect it.
//!
//! The records here mirror the crate's own types instead of exporting them,
//! so that fields that only Rust needs (where an attachment's bytes are, a message's author) stay
//! in Rust.
//!
//! Built with the `ffi` feature only.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use crate::accounts::{AccountFailure as CoreAccountFailure, InboxPage as CoreInboxPage};
use crate::authres::{Authentication as CoreAuthentication, Check as CoreCheck};
use crate::bimi::{BimiResult as CoreBimiResult, Brand as CoreBrand, MarkResult as CoreMarkResult};
use crate::card::{
    Attachment as CoreAttachment, Card as CoreCard, CardBody as CoreCardBody,
    MessageContent as CoreMessageContent, Person as CorePerson,
};
use crate::error::{CommandError, ErrorKind as CoreErrorKind};
use crate::gmail::{Label as CoreLabel, Mailbox as CoreMailbox};
use crate::preferences::Preferences as CorePreferences;
use crate::views::View as CoreView;
use crate::{Core, PendingSignIn};

/// One runtime for the whole process, never dropped: dropping a Tokio
/// runtime from inside one panics, and nothing is gained by shutting it down
/// before the app quits.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("mailcrew-core")
            .build()
            .expect("the Tokio runtime starts")
    })
}

/// Runs `future` on the runtime and waits for it from whatever executor the
/// caller is on.
async fn on_runtime<T: Send + 'static>(
    future: impl Future<Output = crate::error::Result<T>> + Send + 'static,
) -> Result<T, MailCrewError> {
    runtime()
        .spawn(future)
        .await
        .map_err(|err| MailCrewError::Internal {
            message: err.to_string(),
        })?
        .map_err(MailCrewError::from)
}

// --- Errors ------------------------------------------------------------------

/// Why a call failed. The variant is what the app acts on: `SignedOut` sends
/// it back to the sign-in screen,
/// everything else is shown as a notice with `message`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum MailCrewError {
    /// No account is stored, or Google no longer accepts the stored one.
    SignedOut { message: String },
    /// The sign-in was cancelled or timed out before Google redirected back.
    Cancelled { message: String },
    /// The request never got an answer.
    Network { message: String },
    /// Google answered with an error.
    Google { message: String },
    /// The OS credential store refused.
    Keychain { message: String },
    /// Anything else; a bug rather than a condition.
    Internal { message: String },
}

impl std::fmt::Display for MailCrewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (Self::SignedOut { message }
        | Self::Cancelled { message }
        | Self::Network { message }
        | Self::Google { message }
        | Self::Keychain { message }
        | Self::Internal { message }) = self;
        f.write_str(message)
    }
}

impl std::error::Error for MailCrewError {}

impl From<CommandError> for MailCrewError {
    fn from(err: CommandError) -> Self {
        let message = err.message;
        match err.kind {
            CoreErrorKind::SignedOut => Self::SignedOut { message },
            CoreErrorKind::Cancelled => Self::Cancelled { message },
            CoreErrorKind::Network => Self::Network { message },
            CoreErrorKind::Google => Self::Google { message },
            CoreErrorKind::Keychain => Self::Keychain { message },
            CoreErrorKind::Internal => Self::Internal { message },
        }
    }
}

/// What kind of failure an account's part of the inbox met: the variants of
/// `MailCrewError`, without the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ErrorKind {
    SignedOut,
    Cancelled,
    Network,
    Google,
    Keychain,
    Internal,
}

impl From<CoreErrorKind> for ErrorKind {
    fn from(kind: CoreErrorKind) -> Self {
        match kind {
            CoreErrorKind::SignedOut => Self::SignedOut,
            CoreErrorKind::Cancelled => Self::Cancelled,
            CoreErrorKind::Network => Self::Network,
            CoreErrorKind::Google => Self::Google,
            CoreErrorKind::Keychain => Self::Keychain,
            CoreErrorKind::Internal => Self::Internal,
        }
    }
}

// --- Records -----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Person {
    pub name: String,
    pub email: String,
}

impl From<CorePerson> for Person {
    fn from(person: CorePerson) -> Self {
        Self {
            name: person.name,
            email: person.email,
        }
    }
}

/// A thread as a card on the strip. The fields are `card::Card`'s.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Card {
    pub id: String,
    /// The address of the account the thread is in. Every action on the card
    /// goes to this account.
    pub account: String,
    pub subject: String,
    pub from: Person,
    /// Epoch milliseconds of the message shown.
    pub date: i64,
    pub message_count: u32,
    pub unread: bool,
    pub starred: bool,
    /// The message the card shows: the newest one that is not a draft.
    pub message_id: String,
    /// Every message in the thread but drafts; what the actions change.
    pub message_ids: Vec<String>,
    pub snippet: String,
}

impl From<CoreCard> for Card {
    fn from(card: CoreCard) -> Self {
        Self {
            id: card.id,
            account: card.account,
            subject: card.subject,
            from: card.from.into(),
            date: card.date,
            message_count: u32::try_from(card.message_count).unwrap_or(u32::MAX),
            unread: card.unread,
            starred: card.starred,
            message_id: card.message_id,
            message_ids: card.message_ids,
            snippet: card.snippet,
        }
    }
}

/// An account whose part of the inbox could not be fetched.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AccountFailure {
    pub account: String,
    pub kind: ErrorKind,
    pub message: String,
}

impl From<CoreAccountFailure> for AccountFailure {
    fn from(failure: CoreAccountFailure) -> Self {
        Self {
            account: failure.account,
            kind: failure.kind.into(),
            message: failure.message,
        }
    }
}

/// One page of the inbox, across the accounts asked for.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct InboxPage {
    pub cards: Vec<Card>,
    /// The accounts that did not answer. The others' cards are there all the same.
    pub failures: Vec<AccountFailure>,
}

impl From<CoreInboxPage> for InboxPage {
    fn from(page: CoreInboxPage) -> Self {
        Self {
            cards: page.cards.into_iter().map(Into::into).collect(),
            failures: page.failures.into_iter().map(Into::into).collect(),
        }
    }
}

/// A message's body, already sanitised.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum CardBody {
    /// `remote_images` says whether the HTML asks for anything from the
    /// network, which is held back until the user lets it in.
    Html {
        html: String,
        remote_images: bool,
    },
    Text {
        text: String,
    },
    Empty,
}

impl From<CoreCardBody> for CardBody {
    fn from(body: CoreCardBody) -> Self {
        match body {
            CoreCardBody::Html {
                html,
                remote_images,
            } => Self::Html {
                html,
                remote_images,
            },
            CoreCardBody::Text { text } => Self::Text { text },
            CoreCardBody::Empty => Self::Empty,
        }
    }
}

/// A file attached to a message.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Attachment {
    /// The part's id within the message, which names it to
    /// `attachment_to_open` and `save_attachment`.
    pub id: String,
    pub name: String,
    pub mime_type: String,
    /// Bytes.
    pub size: u64,
    /// Whether Mail Crew opens it; otherwise it can only be saved.
    pub openable: bool,
}

impl From<CoreAttachment> for Attachment {
    fn from(attachment: CoreAttachment) -> Self {
        Self {
            id: attachment.id,
            name: attachment.name,
            mime_type: attachment.mime_type,
            size: attachment.size,
            openable: attachment.openable,
        }
    }
}

/// One method's verdict, such as `spf=pass`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Check {
    pub result: String,
    pub domain: String,
}

impl From<CoreCheck> for Check {
    fn from(check: CoreCheck) -> Self {
        Self {
            result: check.result,
            domain: check.domain,
        }
    }
}

/// What Gmail found when it checked the sender; a method it did not report
/// is `None`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Authentication {
    pub spf: Option<Check>,
    pub dkim: Option<Check>,
    pub dmarc: Option<Check>,
}

impl From<CoreAuthentication> for Authentication {
    fn from(authentication: CoreAuthentication) -> Self {
        Self {
            spf: authentication.spf.map(Into::into),
            dkim: authentication.dkim.map(Into::into),
            dmarc: authentication.dmarc.map(Into::into),
        }
    }
}

/// The body of the message a card shows, with its attachments and checks.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MessageContent {
    pub body: CardBody,
    pub attachments: Vec<Attachment>,
    pub authentication: Authentication,
}

impl From<CoreMessageContent> for MessageContent {
    fn from(content: CoreMessageContent) -> Self {
        Self {
            body: content.body.into(),
            attachments: content.attachments.into_iter().map(Into::into).collect(),
            authentication: content.authentication.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BimiResult {
    Pass,
    None,
    Declined,
    Fail,
    Skipped,
    Temperror,
}

impl From<CoreBimiResult> for BimiResult {
    fn from(result: CoreBimiResult) -> Self {
        match result {
            CoreBimiResult::Pass => Self::Pass,
            CoreBimiResult::None => Self::None,
            CoreBimiResult::Declined => Self::Declined,
            CoreBimiResult::Fail => Self::Fail,
            CoreBimiResult::Skipped => Self::Skipped,
            CoreBimiResult::Temperror => Self::Temperror,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MarkResult {
    None,
    Vmc,
    Cmc,
    Fail,
    Temperror,
}

impl From<CoreMarkResult> for MarkResult {
    fn from(result: CoreMarkResult) -> Self {
        match result {
            CoreMarkResult::None => Self::None,
            CoreMarkResult::Vmc => Self::Vmc,
            CoreMarkResult::Cmc => Self::Cmc,
            CoreMarkResult::Fail => Self::Fail,
            CoreMarkResult::Temperror => Self::Temperror,
        }
    }
}

/// The sender's BIMI checks, and its logo when they held up.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Brand {
    pub bimi: BimiResult,
    /// The domain whose BIMI record was used; empty when none was.
    pub domain: String,
    pub mark: MarkResult,
    /// The certificate's logo as a `data:image/svg+xml` URL.
    pub logo: Option<String>,
}

impl From<CoreBrand> for Brand {
    fn from(brand: CoreBrand) -> Self {
        Self {
            bimi: brand.bimi.into(),
            domain: brand.domain,
            mark: brand.mark.into(),
            logo: brand.logo,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Label {
    pub id: String,
    pub name: String,
}

impl From<CoreLabel> for Label {
    fn from(label: CoreLabel) -> Self {
        Self {
            id: label.id,
            name: label.name,
        }
    }
}

/// A custom view: a name, which accounts (every one when empty), and a Gmail
/// search (nothing when empty).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct View {
    pub id: String,
    pub name: String,
    pub accounts: Vec<String>,
    pub query: String,
}

impl From<CoreView> for View {
    fn from(view: CoreView) -> Self {
        Self {
            id: view.id,
            name: view.name,
            accounts: view.accounts,
            query: view.query,
        }
    }
}

impl From<View> for CoreView {
    fn from(view: View) -> Self {
        Self {
            id: view.id,
            name: view.name,
            accounts: view.accounts,
            query: view.query,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct Preferences {
    /// Look up the sender's BIMI record and mark certificate.
    pub brand_logos: bool,
}

impl From<CorePreferences> for Preferences {
    fn from(preferences: CorePreferences) -> Self {
        Self {
            brand_logos: preferences.brand_logos,
        }
    }
}

impl From<Preferences> for CorePreferences {
    fn from(preferences: Preferences) -> Self {
        Self {
            brand_logos: preferences.brand_logos,
        }
    }
}

/// Which pile the strip shows: the inbox, or what "あとで読む" set aside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Mailbox {
    Inbox,
    SetAside,
}

impl From<Mailbox> for CoreMailbox {
    fn from(mailbox: Mailbox) -> Self {
        match mailbox {
            Mailbox::Inbox => Self::Inbox,
            Mailbox::SetAside => Self::SetAside,
        }
    }
}

// --- Functions ---------------------------------------------------------------

/// Whether a link in a message may be handed to the OS: http, https and
/// mailto only. Everything else stays where it is.
#[uniffi::export]
pub fn is_openable_link(url: String) -> bool {
    crate::links::is_openable(&url)
}

/// Where opened attachments are written under the system's temporary
/// folder. The app empties it when it starts.
#[uniffi::export]
pub fn opened_attachments_dir(temp_dir: String) -> String {
    path_string(Core::opened_dir(Path::new(&temp_dir)))
}

fn path_string(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

// --- Objects -----------------------------------------------------------------

/// Everything the app can ask of Mail Crew. One for the app's lifetime.
#[derive(uniffi::Object)]
pub struct MailCrew {
    core: Arc<Core>,
}

#[uniffi::export]
impl MailCrew {
    /// `config_dir` is where the views, the senders whose images are shown
    /// and the preferences are kept: `~/Library/Application Support/
    /// com.ysaeki.mailcrew`, where the Tauri app kept them.
    #[uniffi::constructor]
    pub fn new(config_dir: String) -> Arc<Self> {
        Arc::new(Self {
            core: Arc::new(Core::new(PathBuf::from(config_dir))),
        })
    }

    /// The signed-in addresses, in the order they were added; empty when
    /// nobody is signed in.
    pub async fn account_emails(&self) -> Result<Vec<String>, MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move { core.account_emails().await }).await
    }

    /// Gets a sign-in ready. Open its `url()` in the browser, then await its
    /// `finish()`. Without a client (both empty), the first account's is used.
    pub async fn start_sign_in(
        &self,
        client_id: Option<String>,
        client_secret: Option<String>,
    ) -> Result<Arc<SignIn>, MailCrewError> {
        let core = self.core.clone();
        let pending =
            on_runtime(async move { core.start_sign_in(client_id, client_secret).await }).await?;
        Ok(Arc::new(SignIn {
            core: self.core.clone(),
            url: pending.url.clone(),
            pending: Mutex::new(Some(pending)),
        }))
    }

    /// Gives up the sign-in that is waiting for the browser, if any. Its
    /// `finish()` fails with `Cancelled`.
    pub async fn cancel_sign_in(&self) {
        let core = self.core.clone();
        let _ = on_runtime(async move {
            core.cancel_sign_in().await;
            Ok(())
        })
        .await;
    }

    /// Removes one account from Mail Crew. Its mail on Gmail stays as it is.
    pub async fn sign_out(&self, account: String) -> Result<(), MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move { core.sign_out(&account).await }).await
    }

    /// The threads of `mailbox` across `accounts` (every account when empty),
    /// narrowed by a Gmail search when `query` is not empty.
    pub async fn list_inbox(
        &self,
        accounts: Vec<String>,
        mailbox: Mailbox,
        query: String,
    ) -> Result<InboxPage, MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move {
            core.list_inbox(&accounts, mailbox.into(), &query)
                .await
                .map(Into::into)
        })
        .await
    }

    /// The body and attachments of the message a card shows. Costs Gmail
    /// quota, so ask only for the cards near the screen.
    pub async fn get_message(
        &self,
        account: String,
        message_id: String,
    ) -> Result<MessageContent, MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move {
            core.get_message(&account, &message_id)
                .await
                .map(Into::into)
        })
        .await
    }

    /// The BIMI logo and checks for the sender of the message, or nothing
    /// when the user turned the lookups off.
    pub async fn get_brand(
        &self,
        account: String,
        message_id: String,
    ) -> Result<Option<Brand>, MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move {
            core.get_brand(&account, &message_id)
                .await
                .map(|brand| brand.map(Into::into))
        })
        .await
    }

    /// Writes an attachment that may be opened to a folder of its own under
    /// `opened_dir` and returns its path, for the app to hand to the OS. An
    /// attachment of any other kind fails; it can only be saved.
    pub async fn attachment_to_open(
        &self,
        account: String,
        message_id: String,
        attachment_id: String,
        opened_dir: String,
    ) -> Result<String, MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move {
            core.attachment_to_open(
                &account,
                &message_id,
                &attachment_id,
                Path::new(&opened_dir),
            )
            .await
            .map(path_string)
        })
        .await
    }

    /// Saves an attachment to `dir`, never over another file, and returns the
    /// path it was saved under.
    pub async fn save_attachment(
        &self,
        account: String,
        message_id: String,
        attachment_id: String,
        dir: String,
    ) -> Result<String, MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move {
            core.save_attachment(&account, &message_id, &attachment_id, Path::new(&dir))
                .await
                .map(path_string)
        })
        .await
    }

    pub async fn preferences(&self) -> Result<Preferences, MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move { core.preferences().await.map(Into::into) }).await
    }

    pub async fn save_preferences(&self, preferences: Preferences) -> Result<(), MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move { core.save_preferences(preferences.into()).await }).await
    }

    /// The labels the user made, by name.
    pub async fn list_labels(&self, account: String) -> Result<Vec<Label>, MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move {
            core.list_labels(&account)
                .await
                .map(|labels| labels.into_iter().map(Into::into).collect())
        })
        .await
    }

    pub async fn create_label(
        &self,
        account: String,
        name: String,
    ) -> Result<Label, MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move { core.create_label(&account, &name).await.map(Into::into) }).await
    }

    /// "完了": read, and out of `mailbox`.
    pub async fn archive(
        &self,
        account: String,
        message_ids: Vec<String>,
        mailbox: Mailbox,
    ) -> Result<(), MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move { core.archive(&account, &message_ids, mailbox.into()).await }).await
    }

    /// "タグを追加": tagged, and out of `mailbox`.
    pub async fn apply_label(
        &self,
        account: String,
        message_ids: Vec<String>,
        label_id: String,
        mailbox: Mailbox,
    ) -> Result<(), MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move {
            core.apply_label(&account, &message_ids, &label_id, mailbox.into())
                .await
        })
        .await
    }

    /// "あとで読む": tagged SetAside, and out of the inbox.
    pub async fn set_aside(
        &self,
        account: String,
        message_ids: Vec<String>,
    ) -> Result<(), MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move { core.set_aside(&account, &message_ids).await }).await
    }

    /// Stars the message the card shows, or unstars the whole thread.
    pub async fn set_starred(
        &self,
        account: String,
        message_id: String,
        message_ids: Vec<String>,
        starred: bool,
    ) -> Result<(), MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move {
            core.set_starred(&account, &message_id, &message_ids, starred)
                .await
        })
        .await
    }

    pub async fn list_views(&self) -> Result<Vec<View>, MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move {
            core.list_views()
                .await
                .map(|views| views.into_iter().map(Into::into).collect())
        })
        .await
    }

    /// Replaces every saved view with `views`.
    pub async fn save_views(&self, views: Vec<View>) -> Result<(), MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move {
            core.save_views(views.into_iter().map(Into::into).collect())
                .await
        })
        .await
    }

    /// The senders whose messages show images from the network without asking.
    pub async fn list_image_senders(&self) -> Result<Vec<String>, MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move { core.list_image_senders().await }).await
    }

    /// Replaces every such sender with `senders`, and returns them as saved:
    /// in lower case, each once.
    pub async fn save_image_senders(
        &self,
        senders: Vec<String>,
    ) -> Result<Vec<String>, MailCrewError> {
        let core = self.core.clone();
        on_runtime(async move { core.save_image_senders(senders).await }).await
    }
}

/// A sign-in waiting for the browser.
#[derive(uniffi::Object)]
pub struct SignIn {
    core: Arc<Core>,
    url: String,
    /// Taken by the first `finish()`.
    pending: Mutex<Option<PendingSignIn>>,
}

#[uniffi::export]
impl SignIn {
    /// Google's sign-in page, to open in the default browser.
    pub fn url(&self) -> String {
        self.url.clone()
    }

    /// Waits for Google to redirect back, then stores the account. Resolves
    /// with its address. Only the first call waits; any later one fails.
    pub async fn finish(&self) -> Result<String, MailCrewError> {
        let pending = self
            .pending
            .lock()
            .expect("the lock is never poisoned")
            .take()
            .ok_or_else(|| MailCrewError::Internal {
                message: "このログインはすでに終わっています".to_string(),
            })?;
        let core = self.core.clone();
        on_runtime(async move { core.finish_sign_in(pending).await }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_keep_their_kind_and_message() {
        let err: MailCrewError = CommandError::signed_out().into();
        assert!(matches!(err, MailCrewError::SignedOut { .. }));
        assert_eq!(err.to_string(), CommandError::signed_out().message);
        let err: MailCrewError = CommandError::new(CoreErrorKind::Network, "x").into();
        assert_eq!(
            err,
            MailCrewError::Network {
                message: "x".to_string()
            }
        );
    }

    #[test]
    fn views_and_preferences_round_trip() {
        let view = View {
            id: "v".into(),
            name: "仕事".into(),
            accounts: vec!["a@example.com".into()],
            query: "is:unread".into(),
        };
        assert_eq!(View::from(CoreView::from(view.clone())), view);
        let preferences = Preferences { brand_logos: false };
        assert_eq!(
            Preferences::from(CorePreferences::from(preferences)),
            preferences
        );
    }

    #[test]
    fn links_and_the_opened_folder() {
        assert!(is_openable_link("https://example.com".into()));
        assert!(!is_openable_link("javascript:alert(1)".into()));
        // Joined with the platform's separator, `\` on Windows.
        assert_eq!(
            opened_attachments_dir("/tmp".into()),
            path_string(Path::new("/tmp").join("mailcrew-attachments"))
        );
    }

    #[test]
    fn the_runtime_runs_what_it_is_given() {
        let answer = futures::executor::block_on(on_runtime(async { Ok(42) }));
        assert_eq!(answer, Ok(42));
    }
}
