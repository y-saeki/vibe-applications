//! The signed-in accounts, kept in the OS credential store (the macOS Keychain,
//! the Windows Credential Manager). Nothing about an account is written
//! anywhere else.
//!
//! On macOS every account goes in one entry. The Keychain asks before an
//! updated build reads an entry, once for each entry, so one entry means one
//! question per update however many accounts there are. Windows asks nothing
//! but caps an entry at 2,560 bytes, a handful of accounts, so there each
//! account is an entry of its own, and one more lists their addresses in the
//! order they were added, since the store cannot be asked what it holds.
//! macOS used that layout too before 0.4, and moves out of it once.

use serde::{Deserialize, Serialize};

use crate::error::{CommandError, ErrorKind, Result};

const SERVICE: &str = "com.ysaeki.mailcrew";
/// Every account, as a JSON array of `Account`. macOS only.
#[cfg(target_os = "macos")]
const ALL_USER: &str = "all-accounts";
/// The addresses of the accounts kept one entry each, as a JSON array.
const INDEX_USER: &str = "accounts";
/// Where versions before multiple accounts kept their one account. Read once,
/// moved, and deleted.
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

/// The addresses in `index` that none of `accounts` has.
#[cfg(any(test, not(target_os = "macos")))]
fn dropped(index: &[String], accounts: &[Account]) -> Vec<String> {
    index
        .iter()
        .filter(|email| !accounts.iter().any(|account| &account.email == *email))
        .cloned()
        .collect()
}

/// The accounts kept one entry each, in the order they were added, and
/// whether they came from the entry of a version that knew only one.
fn read_entries() -> Result<(Vec<Account>, bool)> {
    let Some(index) = read_index()? else {
        let Some(json) = read(LEGACY_USER)? else {
            return Ok((Vec::new(), false));
        };
        let account: Account = serde_json::from_str(&json).map_err(broken)?;
        return Ok((vec![account], true));
    };
    let mut accounts = Vec::with_capacity(index.len());
    for email in index {
        // An address whose entry has gone missing is skipped rather than
        // taking every other account down with it.
        if let Some(json) = read(&account_user(&email))? {
            accounts.push(serde_json::from_str(&json).map_err(broken)?);
        }
    }
    Ok((accounts, false))
}

/// Keeps `accounts` one entry each, and deletes the entries of the ones that
/// are no longer there.
#[cfg(not(target_os = "macos"))]
fn write_entries(accounts: &[Account]) -> Result<()> {
    let before = read_index()?.unwrap_or_default();
    for account in accounts {
        write(&account_user(&account.email), &to_json(account)?)?;
    }
    let index: Vec<&str> = accounts
        .iter()
        .map(|account| account.email.as_str())
        .collect();
    write(INDEX_USER, &to_json(&index)?)?;
    for email in dropped(&before, accounts) {
        delete(&account_user(&email))?;
    }
    Ok(())
}

/// The stored accounts, in the order they were added. Accounts kept the way
/// an earlier version kept them are moved to where this one keeps them.
#[cfg(target_os = "macos")]
pub fn load_all() -> Result<Vec<Account>> {
    if let Some(json) = read(ALL_USER)? {
        return serde_json::from_str(&json).map_err(broken);
    }
    let (accounts, legacy) = read_entries()?;
    if accounts.is_empty() {
        return Ok(accounts);
    }
    write(ALL_USER, &to_json(&accounts)?)?;
    // Once the new entry exists the old ones are never read again, so one that
    // the Keychain will not let go of is left behind rather than failing the
    // start-up over it.
    let mut old = if legacy {
        vec![LEGACY_USER.to_string()]
    } else {
        vec![INDEX_USER.to_string()]
    };
    old.extend(accounts.iter().map(|account| account_user(&account.email)));
    for user in old {
        if let Err(err) = delete(&user) {
            eprintln!(
                "mailcrew: could not delete the old entry {user}: {}",
                err.message
            );
        }
    }
    Ok(accounts)
}

/// The stored accounts, in the order they were added. An account kept by a
/// version that knew only one is moved to where this one keeps it.
#[cfg(not(target_os = "macos"))]
pub fn load_all() -> Result<Vec<Account>> {
    let (accounts, legacy) = read_entries()?;
    if legacy {
        write_entries(&accounts)?;
        delete(LEGACY_USER)?;
    }
    Ok(accounts)
}

/// Keeps exactly `accounts`, in this order, in place of what was stored.
pub fn save_all(accounts: &[Account]) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        write(ALL_USER, &to_json(&accounts)?)
    }
    #[cfg(not(target_os = "macos"))]
    {
        write_entries(accounts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(email: &str) -> Account {
        Account {
            client_id: "id".into(),
            client_secret: "s".into(),
            refresh_token: "r".into(),
            email: email.into(),
        }
    }

    fn emails(list: &[&str]) -> Vec<String> {
        list.iter().map(|email| email.to_string()).collect()
    }

    #[test]
    fn addresses_no_account_has_are_dropped() {
        let accounts = [account("a@x"), account("c@x")];
        assert_eq!(
            dropped(&emails(&["a@x", "b@x", "c@x"]), &accounts),
            emails(&["b@x"])
        );
        assert!(dropped(&emails(&["a@x"]), &accounts).is_empty());
    }

    #[test]
    fn accounts_get_entries_apart_from_the_old_one() {
        assert_eq!(account_user("a@x"), "google-account:a@x");
        assert_ne!(account_user("a@x"), LEGACY_USER);
        assert_ne!(account_user("a@x"), INDEX_USER);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_single_entry_is_apart_from_the_old_ones() {
        assert_ne!(ALL_USER, INDEX_USER);
        assert_ne!(ALL_USER, LEGACY_USER);
        assert!(!ALL_USER.starts_with(&account_user("")));
    }

    #[test]
    fn the_single_entry_keeps_the_order() {
        let accounts = vec![account("b@x"), account("a@x")];
        let json = to_json(&accounts).unwrap();
        let read: Vec<Account> = serde_json::from_str(&json).unwrap();
        assert_eq!(read, accounts);
    }

    #[test]
    fn the_old_entry_reads_as_an_account() {
        let json = r#"{"clientId":"id","clientSecret":"s","refreshToken":"r","email":"a@x"}"#;
        let account: Account = serde_json::from_str(json).unwrap();
        assert_eq!(account.email, "a@x");
        assert_eq!(account.refresh_token, "r");
    }
}
