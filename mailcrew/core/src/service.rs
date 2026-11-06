//! Everything a front end can ask of Mail Crew, with nothing about how it is
//! shown. The native app's bindings (`ffi`) are a thin layer over
//! `Core`; what the OS does for the app (opening a URL or a file, where the
//! downloads folder is) stays with the app.

use std::path::{Path, PathBuf};

use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};

use crate::accounts::{Accounts, InboxPage};
use crate::attachments;
use crate::auth::{self, AccessToken, Pkce};
use crate::bimi::{Brand, Brands};
use crate::card::MessageContent;
use crate::credentials::Account;
use crate::error::{CommandError, ErrorKind, Result};
use crate::gmail::{Gmail, Label, Mailbox};
use crate::image_senders;
use crate::preferences::{self, Preferences};
use crate::views::{self, View};

/// The name of the folder, under the system's temporary folder, that opened
/// attachments are written to.
const OPENED_DIR_NAME: &str = "mailcrew-attachments";

pub struct Core {
    accounts: Accounts,
    /// Gives up the sign-in that is waiting for the browser, if any.
    cancel_sign_in: Mutex<Option<oneshot::Sender<()>>>,
    brands: Brands,
    /// Read from disk on first use.
    preferences: Mutex<Option<Preferences>>,
    /// Where the views, the senders whose images are shown and the
    /// preferences are kept.
    config_dir: PathBuf,
}

/// A sign-in that is waiting for the browser: `url` is the page to open, and
/// `Core::finish_sign_in` waits for Google to redirect back.
pub struct PendingSignIn {
    pub url: String,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    listener: TcpListener,
    pkce: Pkce,
    csrf: String,
    cancel: oneshot::Receiver<()>,
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))?
}

impl Core {
    /// `config_dir` is the app's config folder, where the Tauri app kept its
    /// files before the native app took its place.
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            accounts: Accounts::new(),
            cancel_sign_in: Mutex::new(None),
            brands: Brands::new(),
            preferences: Mutex::new(None),
            config_dir,
        }
    }

    /// The signed-in addresses, in the order they were added; empty when
    /// nobody is signed in.
    pub async fn account_emails(&self) -> Result<Vec<String>> {
        self.accounts.emails().await
    }

    /// The client to sign in with: the one given, or the first account's when
    /// both are left empty.
    async fn client_for(
        &self,
        client_id: Option<String>,
        client_secret: Option<String>,
    ) -> Result<(String, String)> {
        let client_id = client_id.unwrap_or_default().trim().to_string();
        let client_secret = client_secret.unwrap_or_default().trim().to_string();
        if client_id.is_empty() && client_secret.is_empty() {
            if let Some(client) = self.accounts.first_client().await? {
                return Ok(client);
            }
        }
        if client_id.is_empty() || client_secret.is_empty() {
            return Err(CommandError::new(
                ErrorKind::Internal,
                "クライアント ID とクライアント シークレットを入力してください",
            ));
        }
        Ok((client_id, client_secret))
    }

    /// Gets a sign-in ready: opens the port Google redirects back to, and
    /// makes the page to open in the browser. Without a client, the one the
    /// first account signed in with is used. A sign-in already waiting is
    /// given up for this one.
    pub async fn start_sign_in(
        &self,
        client_id: Option<String>,
        client_secret: Option<String>,
    ) -> Result<PendingSignIn> {
        let (client_id, client_secret) = self.client_for(client_id, client_secret).await?;
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.map_err(|err| {
            CommandError::new(
                ErrorKind::Internal,
                format!("ログイン用のポートを開けませんでした: {err}"),
            )
        })?;
        let port = listener
            .local_addr()
            .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))?
            .port();
        let redirect_uri = format!("http://127.0.0.1:{port}");
        let pkce = Pkce::new();
        let csrf = auth::random_state();
        let url = auth::authorization_url(&client_id, &redirect_uri, &pkce.challenge, &csrf);

        let (cancel_tx, cancel) = oneshot::channel();
        if let Some(previous) = self.cancel_sign_in.lock().await.replace(cancel_tx) {
            let _ = previous.send(());
        }
        Ok(PendingSignIn {
            url,
            client_id,
            client_secret,
            redirect_uri,
            listener,
            pkce,
            csrf,
            cancel,
        })
    }

    /// Waits for Google to redirect back to `pending`, then stores the
    /// account. Resolves with its address.
    pub async fn finish_sign_in(&self, pending: PendingSignIn) -> Result<String> {
        let PendingSignIn {
            client_id,
            client_secret,
            redirect_uri,
            listener,
            pkce,
            csrf,
            cancel,
            ..
        } = pending;
        let code = tokio::select! {
            code = tokio::time::timeout(auth::SIGN_IN_TIMEOUT, auth::wait_for_code(listener, &csrf)) => {
                code.map_err(|_| CommandError::new(ErrorKind::Cancelled, "ログインが時間切れになりました"))??
            }
            _ = cancel => return Err(CommandError::new(ErrorKind::Cancelled, "ログインを中止しました")),
        };
        self.cancel_sign_in.lock().await.take();

        let http = self.accounts.http();
        let tokens = auth::exchange_code(
            http,
            &client_id,
            &client_secret,
            &code,
            &pkce.verifier,
            &redirect_uri,
        )
        .await?;
        let refresh_token = tokens.refresh_token.clone().ok_or_else(|| {
            CommandError::new(
                ErrorKind::Google,
                "Google がリフレッシュトークンを返しませんでした",
            )
        })?;
        let mut account = Account {
            client_id,
            client_secret,
            refresh_token,
            email: String::new(),
        };
        // Who signed in is only known by asking Gmail with the new token.
        let email = Gmail::new(
            http.clone(),
            account.clone(),
            Some(AccessToken::from_response(&tokens)),
        )
        .email_address()
        .await?;
        account.email = email.clone();
        self.accounts
            .add(account, AccessToken::from_response(&tokens))
            .await?;
        Ok(email)
    }

    pub async fn cancel_sign_in(&self) {
        if let Some(cancel) = self.cancel_sign_in.lock().await.take() {
            let _ = cancel.send(());
        }
    }

    /// Removes one account from Mail Crew. Its mail on Gmail stays as it is.
    pub async fn sign_out(&self, account: &str) -> Result<()> {
        self.accounts.remove(account).await
    }

    /// The threads of `mailbox` across `accounts` (every account when empty),
    /// narrowed by a Gmail search when `query` is not empty.
    pub async fn list_inbox(
        &self,
        accounts: &[String],
        mailbox: Mailbox,
        query: &str,
    ) -> Result<InboxPage> {
        self.accounts.inbox(accounts, mailbox, query).await
    }

    /// The body and attachments of the message a card shows.
    pub async fn get_message(&self, account: &str, message_id: &str) -> Result<MessageContent> {
        let gmail = self.accounts.get(account).await?;
        self.accounts
            .check(account, gmail.message(message_id).await)
            .await
    }

    /// The BIMI logo and checks for the sender of the message a card shows, or
    /// nothing when the user turned the lookups off. Asks the sender's DNS, and
    /// the host of the mark certificate its record names, once an hour at most.
    pub async fn get_brand(&self, account: &str, message_id: &str) -> Result<Option<Brand>> {
        if !self.preferences().await?.brand_logos {
            return Ok(None);
        }
        // Already fetched for the body, so this costs no quota.
        let content = self.get_message(account, message_id).await?;
        // A message without exactly one author is never looked up.
        let author = content.author.unwrap_or_default();
        let passed = !author.is_empty() && content.authentication.dmarc_passed();
        Ok(Some(self.brands.brand(&author, passed).await))
    }

    /// Where opened attachments are written under the system's temporary
    /// folder `temp_dir`. Emptied each time Mail Crew starts.
    pub fn opened_dir(temp_dir: &Path) -> PathBuf {
        temp_dir.join(OPENED_DIR_NAME)
    }

    /// Writes an attachment to a folder of its own under `opened_dir`, for
    /// the OS to open, and returns its path. Only the kinds
    /// `attachments::is_openable` allows are written; the rest can only be
    /// saved.
    pub async fn attachment_to_open(
        &self,
        account: &str,
        message_id: &str,
        attachment_id: &str,
        opened_dir: &Path,
    ) -> Result<PathBuf> {
        let gmail = self.accounts.get(account).await?;
        let (attachment, bytes) = self
            .accounts
            .check(account, gmail.attachment(message_id, attachment_id).await)
            .await?;
        if !attachment.openable {
            return Err(CommandError::new(
                ErrorKind::Internal,
                "この種類のファイルは Mail Crew からは開けません。保存してから開いてください",
            ));
        }
        let dir = opened_dir.join(format!("{:016x}", rand::random::<u64>()));
        write_file(dir, attachment.name, bytes).await
    }

    /// Saves an attachment to `dir`, never over another file, and returns the
    /// path it was saved under.
    pub async fn save_attachment(
        &self,
        account: &str,
        message_id: &str,
        attachment_id: &str,
        dir: &Path,
    ) -> Result<PathBuf> {
        let gmail = self.accounts.get(account).await?;
        let (attachment, bytes) = self
            .accounts
            .check(account, gmail.attachment(message_id, attachment_id).await)
            .await?;
        write_file(dir.to_path_buf(), attachment.name, bytes).await
    }

    pub async fn preferences(&self) -> Result<Preferences> {
        let mut held = self.preferences.lock().await;
        if let Some(preferences) = *held {
            return Ok(preferences);
        }
        let path = self.config_dir.join(preferences::FILE_NAME);
        let loaded = blocking(move || preferences::load(&path)).await?;
        *held = Some(loaded);
        Ok(loaded)
    }

    pub async fn save_preferences(&self, preferences: Preferences) -> Result<()> {
        let mut held = self.preferences.lock().await;
        let path = self.config_dir.join(preferences::FILE_NAME);
        blocking(move || preferences::save(&path, &preferences)).await?;
        *held = Some(preferences);
        Ok(())
    }

    pub async fn list_labels(&self, account: &str) -> Result<Vec<Label>> {
        let gmail = self.accounts.get(account).await?;
        self.accounts.check(account, gmail.labels().await).await
    }

    pub async fn create_label(&self, account: &str, name: &str) -> Result<Label> {
        let gmail = self.accounts.get(account).await?;
        self.accounts
            .check(account, gmail.create_label(name).await)
            .await
    }

    /// "完了": read, and out of `mailbox`.
    pub async fn archive(
        &self,
        account: &str,
        message_ids: &[String],
        mailbox: Mailbox,
    ) -> Result<()> {
        let gmail = self.accounts.get(account).await?;
        let result = async {
            let mut remove = gmail.leaving(mailbox).await?;
            remove.push("UNREAD".to_string());
            let remove: Vec<&str> = remove.iter().map(String::as_str).collect();
            gmail.modify(message_ids, &[], &remove).await
        }
        .await;
        self.accounts.check(account, result).await
    }

    /// "タグを追加": tagged, and out of `mailbox`.
    pub async fn apply_label(
        &self,
        account: &str,
        message_ids: &[String],
        label_id: &str,
        mailbox: Mailbox,
    ) -> Result<()> {
        let gmail = self.accounts.get(account).await?;
        let result = async {
            let remove = gmail.leaving(mailbox).await?;
            let remove: Vec<&str> = remove.iter().map(String::as_str).collect();
            gmail.modify(message_ids, &[label_id], &remove).await
        }
        .await;
        self.accounts.check(account, result).await
    }

    /// "あとで読む": tagged SetAside, and out of the inbox.
    pub async fn set_aside(&self, account: &str, message_ids: &[String]) -> Result<()> {
        let gmail = self.accounts.get(account).await?;
        let result = async {
            let label = gmail.set_aside_label().await?;
            gmail.modify(message_ids, &[&label], &["INBOX"]).await
        }
        .await;
        self.accounts.check(account, result).await
    }

    /// Stars the message the card shows, or unstars the whole thread, which is
    /// what Gmail itself does.
    pub async fn set_starred(
        &self,
        account: &str,
        message_id: &str,
        message_ids: &[String],
        starred: bool,
    ) -> Result<()> {
        let gmail = self.accounts.get(account).await?;
        let result = if starred {
            gmail
                .modify(&[message_id.to_string()], &["STARRED"], &[])
                .await
        } else {
            gmail.modify(message_ids, &[], &["STARRED"]).await
        };
        self.accounts.check(account, result).await
    }

    pub async fn list_views(&self) -> Result<Vec<View>> {
        let path = self.config_dir.join(views::FILE_NAME);
        blocking(move || views::load(&path)).await
    }

    /// Replaces every saved view with `views`.
    pub async fn save_views(&self, views: Vec<View>) -> Result<()> {
        let path = self.config_dir.join(views::FILE_NAME);
        blocking(move || views::save(&path, &views)).await
    }

    /// The senders whose messages show images from the network without asking.
    pub async fn list_image_senders(&self) -> Result<Vec<String>> {
        let path = self.config_dir.join(image_senders::FILE_NAME);
        blocking(move || image_senders::load(&path)).await
    }

    /// Replaces every such sender with `senders`, and returns them as saved: in
    /// lower case, each once.
    pub async fn save_image_senders(&self, senders: Vec<String>) -> Result<Vec<String>> {
        let path = self.config_dir.join(image_senders::FILE_NAME);
        blocking(move || image_senders::save(&path, &senders)).await
    }
}

async fn write_file(dir: PathBuf, name: String, bytes: Vec<u8>) -> Result<PathBuf> {
    tokio::task::spawn_blocking(move || attachments::write_new(&dir, &name, &bytes))
        .await
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))?
        .map_err(|err| {
            CommandError::new(
                ErrorKind::Internal,
                format!("添付ファイルを書き込めませんでした: {err}"),
            )
        })
}
