//! Everything the page can ask of the Rust side.

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::auth::{self, AccessToken, Pkce};
use crate::card::Card;
use crate::credentials::{self, Account};
use crate::error::{CommandError, ErrorKind, Result};
use crate::gmail::Label;
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    /// The signed-in address, or `None` when nobody is signed in.
    pub email: Option<String>,
    /// `std::env::consts::OS`, for the keyboard shortcuts' modifier.
    pub platform: &'static str,
    pub version: String,
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))?
}

#[tauri::command]
pub async fn get_session(app: AppHandle, state: State<'_, AppState>) -> Result<SessionInfo> {
    if state.gmail.account().await.is_none() {
        let stored = blocking(credentials::load).await?;
        state.gmail.set_account(stored, None).await;
    }
    Ok(SessionInfo {
        email: state.gmail.account().await.map(|account| account.email),
        platform: std::env::consts::OS,
        version: app.package_info().version.to_string(),
    })
}

/// Signs in through the browser and keeps the account. Resolves with the
/// address once Google has redirected back and the tokens are stored.
#[tauri::command]
pub async fn sign_in(
    app: AppHandle,
    state: State<'_, AppState>,
    client_id: String,
    client_secret: String,
) -> Result<String> {
    let client_id = client_id.trim().to_string();
    let client_secret = client_secret.trim().to_string();
    if client_id.is_empty() || client_secret.is_empty() {
        return Err(CommandError::new(
            ErrorKind::Internal,
            "クライアント ID とクライアント シークレットを入力してください",
        ));
    }
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

    let tokens = auth::exchange_code(
        state.gmail.http(),
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
    state
        .gmail
        .set_account(
            Some(account.clone()),
            Some(AccessToken::from_response(&tokens)),
        )
        .await;
    let email = match state.gmail.email_address().await {
        Ok(email) => email,
        Err(err) => {
            state.gmail.set_account(None, None).await;
            return Err(err);
        }
    };
    account.email = email.clone();
    let stored = account.clone();
    blocking(move || credentials::store(&stored)).await?;
    state
        .gmail
        .set_account(Some(account), Some(AccessToken::from_response(&tokens)))
        .await;
    Ok(email)
}

#[tauri::command]
pub async fn cancel_sign_in(state: State<'_, AppState>) -> Result<()> {
    if let Some(cancel) = state.cancel_sign_in.lock().await.take() {
        let _ = cancel.send(());
    }
    Ok(())
}

#[tauri::command]
pub async fn sign_out(state: State<'_, AppState>) -> Result<()> {
    state.gmail.set_account(None, None).await;
    blocking(credentials::clear).await
}

#[tauri::command]
pub async fn list_inbox(state: State<'_, AppState>) -> Result<Vec<Card>> {
    state.gmail.inbox().await
}

#[tauri::command]
pub async fn list_labels(state: State<'_, AppState>) -> Result<Vec<Label>> {
    state.gmail.labels().await
}

#[tauri::command]
pub async fn create_label(state: State<'_, AppState>, name: String) -> Result<Label> {
    state.gmail.create_label(&name).await
}

/// "完了": read, and out of the inbox.
#[tauri::command]
pub async fn archive(state: State<'_, AppState>, message_ids: Vec<String>) -> Result<()> {
    state
        .gmail
        .modify(&message_ids, &[], &["INBOX", "UNREAD"])
        .await
}

/// "タグを追加": tagged, and out of the inbox.
#[tauri::command]
pub async fn apply_label(
    state: State<'_, AppState>,
    message_ids: Vec<String>,
    label_id: String,
) -> Result<()> {
    state
        .gmail
        .modify(&message_ids, &[&label_id], &["INBOX"])
        .await
}

/// "あとで読む": tagged SetAside, and out of the inbox.
#[tauri::command]
pub async fn set_aside(state: State<'_, AppState>, message_ids: Vec<String>) -> Result<()> {
    let label = state.gmail.set_aside_label().await?;
    state
        .gmail
        .modify(&message_ids, &[&label], &["INBOX"])
        .await
}

/// Stars the message the card shows, or unstars the whole thread, which is
/// what Gmail itself does.
#[tauri::command]
pub async fn set_starred(
    state: State<'_, AppState>,
    message_id: String,
    message_ids: Vec<String>,
    starred: bool,
) -> Result<()> {
    if starred {
        state.gmail.modify(&[message_id], &["STARRED"], &[]).await
    } else {
        state.gmail.modify(&message_ids, &[], &["STARRED"]).await
    }
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
