//! The signed-in accounts, kept in the OS credential store (the macOS Keychain,
//! the Windows Credential Manager). Each account is one JSON entry of its own,
//! and one more entry lists their addresses in the order they were added, since
//! the store cannot be asked what it holds. Nothing about an account is written
//! anywhere else.

use serde::{Deserialize, Serialize};

use crate::error::{CommandError, ErrorKind, Result};

const SERVICE: &str = "com.ysaeki.mailcrew";
/// The addresses of the stored accounts, as a JSON array.
const INDEX_USER: &str = "accounts";
/// Where versions before multiple accounts kept their one account. Read once,
/// moved to an entry of its own, and deleted.
const LEGACY_USER: &str = "google-account";

/// Everything needed to get a fresh access token without asking again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,
    pub email: String,
}

fn account_user(email: &str) -> String {
    format!("{LEGACY_USER}:{email}")
}

fn entry(user: &str) -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(SERVICE, user)?)
}

fn read(user: &str) -> Result<Option<String>> {
    match entry(user)?.get_password() {
        Ok(json) => Ok(Some(json)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn write(user: &str, json: &str) -> Result<()> {
    entry(user)?.set_password(json)?;
    Ok(())
}

fn delete(user: &str) -> Result<()> {
    match entry(user)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

fn broken(err: serde_json::Error) -> CommandError {
    CommandError::new(
        ErrorKind::Keychain,
        format!("保存されたログイン情報が壊れています: {err}"),
    )
}

fn to_json(value: &impl Serialize) -> Result<String> {
    serde_json::to_string(value)
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))
}

fn read_index() -> Result<Option<Vec<String>>> {
    read(INDEX_USER)?
        .map(|json| serde_json::from_str(&json).map_err(broken))
        .transpose()
}

/// `index` with `email` at the end, unless it is already there.
pub fn with_email(mut index: Vec<String>, email: &str) -> Vec<String> {
    if !index.iter().any(|known| known == email) {
        index.push(email.to_string());
    }
    index
}

/// `index` without `email`.
pub fn without_email(mut index: Vec<String>, email: &str) -> Vec<String> {
    index.retain(|known| known != email);
    index
}

/// The stored accounts, in the order they were added. An account kept by a
/// version that knew only one is moved to where this one keeps it.
pub fn load_all() -> Result<Vec<Account>> {
    let Some(index) = read_index()? else {
        let Some(json) = read(LEGACY_USER)? else {
            return Ok(Vec::new());
        };
        let account: Account = serde_json::from_str(&json).map_err(broken)?;
        store(&account)?;
        delete(LEGACY_USER)?;
        return Ok(vec![account]);
    };
    let mut accounts = Vec::with_capacity(index.len());
    for email in index {
        // An address whose entry has gone missing is skipped rather than
        // taking every other account down with it.
        if let Some(json) = read(&account_user(&email))? {
            accounts.push(serde_json::from_str(&json).map_err(broken)?);
        }
    }
    Ok(accounts)
}

/// Keeps `account`, replacing what was stored for the same address.
pub fn store(account: &Account) -> Result<()> {
    write(&account_user(&account.email), &to_json(account)?)?;
    let index = with_email(read_index()?.unwrap_or_default(), &account.email);
    write(INDEX_USER, &to_json(&index)?)
}

/// Forgets the account with this address.
pub fn remove(email: &str) -> Result<()> {
    let index = without_email(read_index()?.unwrap_or_default(), email);
    write(INDEX_USER, &to_json(&index)?)?;
    delete(&account_user(email))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emails(list: &[&str]) -> Vec<String> {
        list.iter().map(|email| email.to_string()).collect()
    }

    #[test]
    fn a_new_address_goes_at_the_end() {
        assert_eq!(with_email(emails(&["a@x"]), "b@x"), emails(&["a@x", "b@x"]));
    }

    #[test]
    fn signing_in_again_keeps_the_place() {
        assert_eq!(
            with_email(emails(&["a@x", "b@x"]), "a@x"),
            emails(&["a@x", "b@x"])
        );
    }

    #[test]
    fn removing_keeps_the_rest_in_order() {
        assert_eq!(
            without_email(emails(&["a@x", "b@x", "c@x"]), "b@x"),
            emails(&["a@x", "c@x"])
        );
        assert_eq!(without_email(emails(&["a@x"]), "z@x"), emails(&["a@x"]));
    }

    #[test]
    fn accounts_get_entries_apart_from_the_old_one() {
        assert_eq!(account_user("a@x"), "google-account:a@x");
        assert_ne!(account_user("a@x"), LEGACY_USER);
        assert_ne!(account_user("a@x"), INDEX_USER);
    }

    #[test]
    fn the_old_entry_reads_as_an_account() {
        let json = r#"{"clientId":"id","clientSecret":"s","refreshToken":"r","email":"a@x"}"#;
        let account: Account = serde_json::from_str(json).unwrap();
        assert_eq!(account.email, "a@x");
        assert_eq!(account.refresh_token, "r");
    }
}
