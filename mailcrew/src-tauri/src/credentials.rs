//! The signed-in account, kept in the OS credential store (the macOS Keychain,
//! the Windows Credential Manager) as one JSON entry. Nothing about the account
//! is written anywhere else.

use serde::{Deserialize, Serialize};

use crate::error::{CommandError, ErrorKind, Result};

const SERVICE: &str = "com.ysaeki.mailcrew";
const USER: &str = "google-account";

/// Everything needed to get a fresh access token without asking again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,
    pub email: String,
}

fn entry() -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(SERVICE, USER)?)
}

/// The stored account, or `None` when nobody has signed in.
pub fn load() -> Result<Option<Account>> {
    match entry()?.get_password() {
        Ok(json) => serde_json::from_str(&json).map(Some).map_err(|err| {
            CommandError::new(
                ErrorKind::Keychain,
                format!("保存されたログイン情報が壊れています: {err}"),
            )
        }),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub fn store(account: &Account) -> Result<()> {
    let json = serde_json::to_string(account)
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))?;
    entry()?.set_password(&json)?;
    Ok(())
}

pub fn clear() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(err.into()),
    }
}
