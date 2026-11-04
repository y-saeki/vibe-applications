//! Everything the page can ask of the Rust side.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::accounts::InboxPage;
use crate::attachments;
use crate::auth::{self, AccessToken, Pkce};
use crate::bimi::Brand;
use crate::card::MessageContent;
use crate::credentials::Account;
use crate::error::{CommandError, ErrorKind, Result};
use crate::gmail::{Gmail, Label, Mailbox};
use crate::image_senders;
use crate::preferences::{self, Preferences};
use crate::views::{self, View};
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    /// The signed-in addresses, in the order they were added; empty when
    /// nobody is signed in.
    pub accounts: Vec<String>,
    /// `std::env::consts::OS`, for the keyboard shortcuts' modifier.
    pub platform: &'static str,
    pub version: String,
}

#[tauri::command]
pub async fn get_session(app: AppHandle, state: State<'_, AppState>) -> Result<SessionInfo> {
    Ok(SessionInfo {
        accounts: state.accounts.emails().await?,
        platform: std::env::consts::OS,
        version: app.package_info().version.to_string(),
    })
}

/// The client to sign in with: the one given, or the first account's when
/// both are left empty.
async fn client_for(
    state: &AppState,
    client_id: Option<String>,
    client_secret: Option<String>,
) -> Result<(String, String)> {
    let client_id = client_id.unwrap_or_default().trim().to_string();
    let client_secret = client_secret.unwrap_or_default().trim().to_string();
    if client_id.is_empty() && client_secret.is_empty() {
        if let Some(client) = state.accounts.first_client().await? {
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

/// Signs in through the browser and adds the account. Resolves with the
/// address once Google has redirected back and the tokens are stored. Without
/// a client, the one the first account signed in with is used.
#[tauri::command]
pub async fn sign_in(
    app: AppHandle,
    state: State<'_, AppState>,
    client_id: Option<String>,
    client_secret: Option<String>,
) -> Result<String> {
    let (client_id, client_secret) = client_for(&state, client_id, client_secret).await?;
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

    let (cancel_tx, cancel_rx) = oneshot::channel();
    // A sign-in already waiting is given up for this one.
    if let Some(previous) = state.cancel_sign_in.lock().await.replace(cancel_tx) {
        let _ = previous.send(());
    }
    app.opener().open_url(url, None::<&str>).map_err(|err| {
        CommandError::new(
            ErrorKind::Internal,
            format!("ブラウザを開けませんでした: {err}"),
        )
    })?;

    let code = tokio::select! {
        code = tokio::time::timeout(auth::SIGN_IN_TIMEOUT, auth::wait_for_code(listener, &csrf)) => {
            code.map_err(|_| CommandError::new(ErrorKind::Cancelled, "ログインが時間切れになりました"))??
        }
        _ = cancel_rx => return Err(CommandError::new(ErrorKind::Cancelled, "ログインを中止しました")),
    };
    state.cancel_sign_in.lock().await.take();

    let http = state.accounts.http();
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
    state
        .accounts
        .add(account, AccessToken::from_response(&tokens))
        .await?;
    Ok(email)
}

#[tauri::command]
pub async fn cancel_sign_in(state: State<'_, AppState>) -> Result<()> {
    if let Some(cancel) = state.cancel_sign_in.lock().await.take() {
        let _ = cancel.send(());
    }
    Ok(())
}

/// Removes one account from Mail Crew. Its mail on Gmail stays as it is.
#[tauri::command]
pub async fn sign_out(state: State<'_, AppState>, account: String) -> Result<()> {
    state.accounts.remove(&account).await
}

/// The threads of `mailbox` across `accounts` (every account when empty),
/// narrowed by a Gmail search when `query` is not empty.
#[tauri::command]
pub async fn list_inbox(
    state: State<'_, AppState>,
    accounts: Vec<String>,
    mailbox: Option<Mailbox>,
    query: String,
) -> Result<InboxPage> {
    state
        .accounts
        .inbox(&accounts, mailbox.unwrap_or_default(), &query)
        .await
}

/// The body and attachments of the message a card shows.
#[tauri::command]
pub async fn get_message(
    state: State<'_, AppState>,
    account: String,
    message_id: String,
) -> Result<MessageContent> {
    let gmail = state.accounts.get(&account).await?;
    state
        .accounts
        .check(&account, gmail.message(&message_id).await)
        .await
}

/// The BIMI logo and checks for the sender of the message a card shows, or
/// nothing when the user turned the lookups off. Asks the sender's DNS, and
/// the host of the mark certificate its record names, once an hour at most.
#[tauri::command]
pub async fn get_brand(
    app: AppHandle,
    state: State<'_, AppState>,
    account: String,
    message_id: String,
) -> Result<Option<Brand>> {
    if !current_preferences(&app, &state).await?.brand_logos {
        return Ok(None);
    }
    let gmail = state.accounts.get(&account).await?;
    // Already fetched for the body, so this costs no quota.
    let content = state
        .accounts
        .check(&account, gmail.message(&message_id).await)
        .await?;
    // A message without exactly one author is never looked up.
    let author = content.author.unwrap_or_default();
    let passed = !author.is_empty() && content.authentication.dmarc_passed();
    Ok(Some(state.brands.brand(&author, passed).await))
}

/// Opens an attachment in the app the OS picks for it, from a folder of its
/// own under `opened_dir`. Only the kinds `attachments::is_openable` allows
/// are opened; the rest can only be saved.
#[tauri::command]
pub async fn open_attachment(
    app: AppHandle,
    state: State<'_, AppState>,
    account: String,
    message_id: String,
    attachment_id: String,
) -> Result<()> {
    let gmail = state.accounts.get(&account).await?;
    let (attachment, bytes) = state
        .accounts
        .check(
            &account,
            gmail.attachment(&message_id, &attachment_id).await,
        )
        .await?;
    if !attachment.openable {
        return Err(CommandError::new(
            ErrorKind::Internal,
            "この種類のファイルは Mail Crew からは開けません。保存してから開いてください",
        ));
    }
    let dir = opened_dir(&app)?.join(format!("{:016x}", rand::random::<u64>()));
    let path = write_file(dir, attachment.name, bytes).await?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|err| {
            CommandError::new(
                ErrorKind::Internal,
                format!("添付ファイルを開けませんでした: {err}"),
            )
        })
}

/// Saves an attachment to the downloads folder, never over another file, and
/// resolves with the name it was saved under.
#[tauri::command]
pub async fn save_attachment(
    app: AppHandle,
    state: State<'_, AppState>,
    account: String,
    message_id: String,
    attachment_id: String,
) -> Result<String> {
    let gmail = state.accounts.get(&account).await?;
    let (attachment, bytes) = state
        .accounts
        .check(
            &account,
            gmail.attachment(&message_id, &attachment_id).await,
        )
        .await?;
    let dir = app.path().download_dir().map_err(|err| {
        CommandError::new(
            ErrorKind::Internal,
            format!("ダウンロード フォルダが見つかりません: {err}"),
        )
    })?;
    let path = write_file(dir, attachment.name, bytes).await?;
    Ok(path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default())
}

/// Where opened attachments are written, emptied each time Mail Crew starts.
pub fn opened_dir(app: &AppHandle) -> Result<PathBuf> {
    app.path()
        .temp_dir()
        .map(|dir| dir.join("mailcrew-attachments"))
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))
}

async fn write_file(dir: PathBuf, name: String, bytes: Vec<u8>) -> Result<PathBuf> {
    tauri::async_runtime::spawn_blocking(move || attachments::write_new(&dir, &name, &bytes))
        .await
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))?
        .map_err(|err| {
            CommandError::new(
                ErrorKind::Internal,
                format!("添付ファイルを書き込めませんでした: {err}"),
            )
        })
}

async fn current_preferences(app: &AppHandle, state: &AppState) -> Result<Preferences> {
    let mut held = state.preferences.lock().await;
    if let Some(preferences) = *held {
        return Ok(preferences);
    }
    let path = config_path(app, preferences::FILE_NAME)?;
    let loaded = tauri::async_runtime::spawn_blocking(move || preferences::load(&path))
        .await
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))??;
    *held = Some(loaded);
    Ok(loaded)
}

#[tauri::command]
pub async fn get_preferences(app: AppHandle, state: State<'_, AppState>) -> Result<Preferences> {
    current_preferences(&app, &state).await
}

#[tauri::command]
pub async fn save_preferences(
    app: AppHandle,
    state: State<'_, AppState>,
    preferences: Preferences,
) -> Result<()> {
    let mut held = state.preferences.lock().await;
    let path = config_path(&app, preferences::FILE_NAME)?;
    tauri::async_runtime::spawn_blocking(move || preferences::save(&path, &preferences))
        .await
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))??;
    *held = Some(preferences);
    Ok(())
}

#[tauri::command]
pub async fn list_labels(state: State<'_, AppState>, account: String) -> Result<Vec<Label>> {
    let gmail = state.accounts.get(&account).await?;
    state.accounts.check(&account, gmail.labels().await).await
}

#[tauri::command]
pub async fn create_label(
    state: State<'_, AppState>,
    account: String,
    name: String,
) -> Result<Label> {
    let gmail = state.accounts.get(&account).await?;
    state
        .accounts
        .check(&account, gmail.create_label(&name).await)
        .await
}

/// "完了": read, and out of `mailbox` (the inbox when not given).
#[tauri::command]
pub async fn archive(
    state: State<'_, AppState>,
    account: String,
    message_ids: Vec<String>,
    mailbox: Option<Mailbox>,
) -> Result<()> {
    let gmail = state.accounts.get(&account).await?;
    let result = async {
        let mut remove = gmail.leaving(mailbox.unwrap_or_default()).await?;
        remove.push("UNREAD".to_string());
        let remove: Vec<&str> = remove.iter().map(String::as_str).collect();
        gmail.modify(&message_ids, &[], &remove).await
    }
    .await;
    state.accounts.check(&account, result).await
}

/// "タグを追加": tagged, and out of `mailbox` (the inbox when not given).
#[tauri::command]
pub async fn apply_label(
    state: State<'_, AppState>,
    account: String,
    message_ids: Vec<String>,
    label_id: String,
    mailbox: Option<Mailbox>,
) -> Result<()> {
    let gmail = state.accounts.get(&account).await?;
    let result = async {
        let remove = gmail.leaving(mailbox.unwrap_or_default()).await?;
        let remove: Vec<&str> = remove.iter().map(String::as_str).collect();
        gmail.modify(&message_ids, &[&label_id], &remove).await
    }
    .await;
    state.accounts.check(&account, result).await
}

/// "あとで読む": tagged SetAside, and out of the inbox.
#[tauri::command]
pub async fn set_aside(
    state: State<'_, AppState>,
    account: String,
    message_ids: Vec<String>,
) -> Result<()> {
    let gmail = state.accounts.get(&account).await?;
    let result = async {
        let label = gmail.set_aside_label().await?;
        gmail.modify(&message_ids, &[&label], &["INBOX"]).await
    }
    .await;
    state.accounts.check(&account, result).await
}

/// Stars the message the card shows, or unstars the whole thread, which is
/// what Gmail itself does.
#[tauri::command]
pub async fn set_starred(
    state: State<'_, AppState>,
    account: String,
    message_id: String,
    message_ids: Vec<String>,
    starred: bool,
) -> Result<()> {
    let gmail = state.accounts.get(&account).await?;
    let result = if starred {
        gmail.modify(&[message_id], &["STARRED"], &[]).await
    } else {
        gmail.modify(&message_ids, &[], &["STARRED"]).await
    };
    state.accounts.check(&account, result).await
}

fn config_path(app: &AppHandle, file: &str) -> Result<PathBuf> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join(file))
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))
}

fn views_path(app: &AppHandle) -> Result<PathBuf> {
    config_path(app, views::FILE_NAME)
}

#[tauri::command]
pub async fn list_views(app: AppHandle) -> Result<Vec<View>> {
    let path = views_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || views::load(&path))
        .await
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))?
}

/// Replaces every saved view with `views`.
#[tauri::command]
pub async fn save_views(app: AppHandle, views: Vec<View>) -> Result<()> {
    let path = views_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || views::save(&path, &views))
        .await
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))?
}

/// The senders whose messages show images from the network without asking.
#[tauri::command]
pub async fn list_image_senders(app: AppHandle) -> Result<Vec<String>> {
    let path = config_path(&app, image_senders::FILE_NAME)?;
    tauri::async_runtime::spawn_blocking(move || image_senders::load(&path))
        .await
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))?
}

/// Replaces every such sender with `senders`, and returns them as saved: in
/// lower case, each once.
#[tauri::command]
pub async fn save_image_senders(app: AppHandle, senders: Vec<String>) -> Result<Vec<String>> {
    let path = config_path(&app, image_senders::FILE_NAME)?;
    tauri::async_runtime::spawn_blocking(move || image_senders::save(&path, &senders))
        .await
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))?
}

/// Whether a link in a message may be handed to the OS.
pub fn is_openable(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https" | "mailto"))
}

/// Opens a link from a message in the default browser or mail client.
#[tauri::command]
pub fn open_link(app: AppHandle, url: String) -> Result<()> {
    if !is_openable(&url) {
        return Err(CommandError::new(
            ErrorKind::Internal,
            "このリンクは開けません",
        ));
    }
    app.opener().open_url(url, None::<&str>).map_err(|err| {
        CommandError::new(
            ErrorKind::Internal,
            format!("リンクを開けませんでした: {err}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::is_openable;

    #[test]
    fn only_web_and_mail_links_open() {
        assert!(is_openable("https://example.com/x?y=1"));
        assert!(is_openable("http://example.com"));
        assert!(is_openable("mailto:a@example.com"));
        assert!(!is_openable("javascript:alert(1)"));
        assert!(!is_openable("file:///etc/passwd"));
        assert!(!is_openable("data:text/html,<script>alert(1)</script>"));
        assert!(!is_openable("smb://host/share"));
        assert!(!is_openable("not a url"));
    }
}
