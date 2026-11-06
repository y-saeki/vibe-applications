//! The one error type every call into `Core` returns. `ffi` turns it into
//! `MailCrewError`, and `kind` is what the app acts on: `signedOut` sends it
//! back to the sign-in screen, everything else is shown as a notice.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    /// No account is stored, or Google no longer accepts the stored one.
    SignedOut,
    /// The sign-in was cancelled or timed out before Google redirected back.
    Cancelled,
    /// The request never got an answer.
    Network,
    /// Google answered with an error.
    Google,
    /// The OS credential store refused.
    Keychain,
    /// Anything else; a bug rather than a condition.
    Internal,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandError {
    pub kind: ErrorKind,
    pub message: String,
}

pub type Result<T> = std::result::Result<T, CommandError>;

impl CommandError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn signed_out() -> Self {
        Self::new(
            ErrorKind::SignedOut,
            "Google アカウントにログインしていません",
        )
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<reqwest::Error> for CommandError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_decode() {
            Self::new(
                ErrorKind::Google,
                format!("Google の応答を読み取れませんでした: {err}"),
            )
        } else {
            Self::new(
                ErrorKind::Network,
                format!("Google に接続できませんでした: {err}"),
            )
        }
    }
}

impl From<keyring::Error> for CommandError {
    fn from(err: keyring::Error) -> Self {
        Self::new(
            ErrorKind::Keychain,
            format!("ログイン情報を読み書きできませんでした: {err}"),
        )
    }
}
