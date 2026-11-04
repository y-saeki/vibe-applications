//! Everything the page can ask of the Rust side. Each command hands its work
//! to `mailcrew_core::Core` and does here only what the OS does for it:
//! opening the browser or a file, and finding the downloads folder.

use std::path::PathBuf;

use mailcrew_core::accounts::InboxPage;
use mailcrew_core::bimi::Brand;
use mailcrew_core::card::MessageContent;
use mailcrew_core::error::{CommandError, ErrorKind, Result};
use mailcrew_core::gmail::{Label, Mailbox};
use mailcrew_core::links::is_openable;
use mailcrew_core::preferences::Preferences;
use mailcrew_core::views::View;
use mailcrew_core::Core;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

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
pub async fn get_session(app: AppHandle, core: State<'_, Core>) -> Result<SessionInfo> {
    Ok(SessionInfo {
        accounts: core.account_emails().await?,
        platform: std::env::consts::OS,
        version: app.package_info().version.to_string(),
    })
}

/// Signs in through the browser and adds the account. Resolves with the
/// address once Google has redirected back and the tokens are stored. Without
/// a client, the one the first account signed in with is used.
#[tauri::command]
pub async fn sign_in(
    app: AppHandle,
    core: State<'_, Core>,
    client_id: Option<String>,
    client_secret: Option<String>,
) -> Result<String> {
    let pending = core.start_sign_in(client_id, client_secret).await?;
    app.opener()
        .open_url(pending.url.clone(), None::<&str>)
        .map_err(|err| {
            CommandError::new(
                ErrorKind::Internal,
                format!("ブラウザを開けませんでした: {err}"),
            )
        })?;
    core.finish_sign_in(pending).await
}

#[tauri::command]
pub async fn cancel_sign_in(core: State<'_, Core>) -> Result<()> {
    core.cancel_sign_in().await;
    Ok(())
}

/// Removes one account from Mail Crew. Its mail on Gmail stays as it is.
#[tauri::command]
pub async fn sign_out(core: State<'_, Core>, account: String) -> Result<()> {
    core.sign_out(&account).await
}

/// The threads of `mailbox` across `accounts` (every account when empty),
/// narrowed by a Gmail search when `query` is not empty.
#[tauri::command]
pub async fn list_inbox(
    core: State<'_, Core>,
    accounts: Vec<String>,
    mailbox: Option<Mailbox>,
    query: String,
) -> Result<InboxPage> {
    core.list_inbox(&accounts, mailbox.unwrap_or_default(), &query)
        .await
}

/// The body and attachments of the message a card shows.
#[tauri::command]
pub async fn get_message(
    core: State<'_, Core>,
    account: String,
    message_id: String,
) -> Result<MessageContent> {
    core.get_message(&account, &message_id).await
}

/// The BIMI logo and checks for the sender of the message a card shows, or
/// nothing when the user turned the lookups off.
#[tauri::command]
pub async fn get_brand(
    core: State<'_, Core>,
    account: String,
    message_id: String,
) -> Result<Option<Brand>> {
    core.get_brand(&account, &message_id).await
}

/// Opens an attachment in the app the OS picks for it, from a folder of its
/// own under `opened_dir`. Only the kinds `attachments::is_openable` allows
/// are opened; the rest can only be saved.
#[tauri::command]
pub async fn open_attachment(
    app: AppHandle,
    core: State<'_, Core>,
    account: String,
    message_id: String,
    attachment_id: String,
) -> Result<()> {
    let path = core
        .attachment_to_open(&account, &message_id, &attachment_id, &opened_dir(&app)?)
        .await?;
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
    core: State<'_, Core>,
    account: String,
    message_id: String,
    attachment_id: String,
) -> Result<String> {
    let dir = app.path().download_dir().map_err(|err| {
        CommandError::new(
            ErrorKind::Internal,
            format!("ダウンロード フォルダが見つかりません: {err}"),
        )
    })?;
    let path = core
        .save_attachment(&account, &message_id, &attachment_id, &dir)
        .await?;
    Ok(path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default())
}

/// Where opened attachments are written, emptied each time Mail Crew starts.
pub fn opened_dir(app: &AppHandle) -> Result<PathBuf> {
    app.path()
        .temp_dir()
        .map(|dir| Core::opened_dir(&dir))
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))
}

#[tauri::command]
pub async fn get_preferences(core: State<'_, Core>) -> Result<Preferences> {
    core.preferences().await
}

#[tauri::command]
pub async fn save_preferences(core: State<'_, Core>, preferences: Preferences) -> Result<()> {
    core.save_preferences(preferences).await
}

#[tauri::command]
pub async fn list_labels(core: State<'_, Core>, account: String) -> Result<Vec<Label>> {
    core.list_labels(&account).await
}

#[tauri::command]
pub async fn create_label(core: State<'_, Core>, account: String, name: String) -> Result<Label> {
    core.create_label(&account, &name).await
}

/// "完了": read, and out of `mailbox` (the inbox when not given).
#[tauri::command]
pub async fn archive(
    core: State<'_, Core>,
    account: String,
    message_ids: Vec<String>,
    mailbox: Option<Mailbox>,
) -> Result<()> {
    core.archive(&account, &message_ids, mailbox.unwrap_or_default())
        .await
}

/// "タグを追加": tagged, and out of `mailbox` (the inbox when not given).
#[tauri::command]
pub async fn apply_label(
    core: State<'_, Core>,
    account: String,
    message_ids: Vec<String>,
    label_id: String,
    mailbox: Option<Mailbox>,
) -> Result<()> {
    core.apply_label(
        &account,
        &message_ids,
        &label_id,
        mailbox.unwrap_or_default(),
    )
    .await
}

/// "あとで読む": tagged SetAside, and out of the inbox.
#[tauri::command]
pub async fn set_aside(
    core: State<'_, Core>,
    account: String,
    message_ids: Vec<String>,
) -> Result<()> {
    core.set_aside(&account, &message_ids).await
}

/// Stars the message the card shows, or unstars the whole thread, which is
/// what Gmail itself does.
#[tauri::command]
pub async fn set_starred(
    core: State<'_, Core>,
    account: String,
    message_id: String,
    message_ids: Vec<String>,
    starred: bool,
) -> Result<()> {
    core.set_starred(&account, &message_id, &message_ids, starred)
        .await
}

#[tauri::command]
pub async fn list_views(core: State<'_, Core>) -> Result<Vec<View>> {
    core.list_views().await
}

/// Replaces every saved view with `views`.
#[tauri::command]
pub async fn save_views(core: State<'_, Core>, views: Vec<View>) -> Result<()> {
    core.save_views(views).await
}

/// The senders whose messages show images from the network without asking.
#[tauri::command]
pub async fn list_image_senders(core: State<'_, Core>) -> Result<Vec<String>> {
    core.list_image_senders().await
}

/// Replaces every such sender with `senders`, and returns them as saved: in
/// lower case, each once.
#[tauri::command]
pub async fn save_image_senders(
    core: State<'_, Core>,
    senders: Vec<String>,
) -> Result<Vec<String>> {
    core.save_image_senders(senders).await
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
