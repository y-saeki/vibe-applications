//! The signed-in accounts. On macOS they are a file in the app's config
//! folder, readable only by the user; on Windows they are in the Credential
//! Manager. Nothing about an account is written anywhere else.
//!
//! macOS kept them in the Keychain up to 0.8.0. The Keychain tells apps apart by
//! their signature, and an app not signed with a certificate Apple issued is
//! told apart by the hash of its code, so it asked for the password after
//! every update. Thunderbird, without a Primary Password, keeps its sign-ins
//! the same way as this file. The Keychain is read once, moved to the file and
//! emptied; see `from_keychain` for the layouts it may be in.
//!
//! Windows asks nothing but caps an entry at 2,560 bytes, a handful of
//! accounts, so there each account is an entry of its own, and one more lists
//! their addresses in the order they were added, since the store cannot be
//! asked what it holds.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{CommandError, ErrorKind, Result};

/// Every account, as a JSON array of `Account`, in the config folder. macOS
/// only.
#[cfg(target_os = "macos")]
pub const FILE_NAME: &str = "accounts.json";

const SERVICE: &str = "com.ysaeki.mailcrew";
/// Where macOS 0.4 to 0.8.0 kept every account, as the file has them now.
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

#[cfg(not(target_os = "macos"))]
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

/// The accounts in the Keychain, wherever an earlier version put them, and
/// the entries they were read from.
#[cfg(target_os = "macos")]
fn from_keychain() -> Result<(Vec<Account>, Vec<String>)> {
    if let Some(json) = read(ALL_USER)? {
        let accounts = serde_json::from_str(&json).map_err(broken)?;
        return Ok((accounts, vec![ALL_USER.to_string()]));
    }
    // Before 0.4 macOS kept the Windows layout.
    let (accounts, legacy) = read_entries()?;
    if accounts.is_empty() {
        return Ok((accounts, Vec::new()));
    }
    let mut old = if legacy {
        vec![LEGACY_USER.to_string()]
    } else {
        vec![INDEX_USER.to_string()]
    };
    old.extend(accounts.iter().map(|account| account_user(&account.email)));
    Ok((accounts, old))
}

#[cfg(target_os = "macos")]
fn file_failed(what: &str, err: impl std::fmt::Display) -> CommandError {
    CommandError::new(
        ErrorKind::Internal,
        format!("ログイン情報を{what}できませんでした: {err}"),
    )
}

#[cfg(target_os = "macos")]
fn read_file(path: &Path) -> Result<Option<Vec<Account>>> {
    match std::fs::read_to_string(path) {
        Ok(json) => Ok(Some(serde_json::from_str(&json).map_err(broken)?)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(file_failed("読み込み", err)),
    }
}

/// Writes `accounts` to `path`, readable and writable by the user alone.
#[cfg(target_os = "macos")]
fn write_file(path: &Path, accounts: &[Account]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let failed = |err: std::io::Error| file_failed("保存", err);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(failed)?;
    }
    // Written beside the file and moved over it, so a crash halfway leaves the
    // old file rather than half of the new one. The mode is set again in case
    // a partial file was left with another one.
    let partial = path.with_extension("json.partial");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&partial)
        .map_err(failed)?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(failed)?;
    file.write_all(to_json(&accounts)?.as_bytes())
        .map_err(failed)?;
    file.sync_all().map_err(failed)?;
    std::fs::rename(&partial, path).map_err(failed)
}

/// The stored accounts in `config_dir`, in the order they were added.
/// Accounts still in the Keychain are moved to the file first.
#[cfg(target_os = "macos")]
pub fn load_all(config_dir: &Path) -> Result<Vec<Account>> {
    let path = config_dir.join(FILE_NAME);
    if let Some(accounts) = read_file(&path)? {
        return Ok(accounts);
    }
    let (accounts, old) = from_keychain()?;
    if old.is_empty() {
        return Ok(accounts);
    }
    write_file(&path, &accounts)?;
    // Once the file exists the Keychain is never read again, so an entry that
    // it will not let go of is left behind rather than failing the start-up
    // over it.
    for user in old {
        if let Err(err) = delete(&user) {
            eprintln!(
                "mailcrew: could not delete the Keychain entry {user}: {}",
                err.message
            );
        }
    }
    Ok(accounts)
}

/// The stored accounts, in the order they were added. An account kept by a
/// version that knew only one is moved to where this one keeps it.
#[cfg(not(target_os = "macos"))]
pub fn load_all(_config_dir: &Path) -> Result<Vec<Account>> {
    let (accounts, legacy) = read_entries()?;
    if legacy {
        write_entries(&accounts)?;
        delete(LEGACY_USER)?;
    }
    Ok(accounts)
}

/// Keeps exactly `accounts`, in this order, in place of what was stored.
pub fn save_all(config_dir: &Path, accounts: &[Account]) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        write_file(&config_dir.join(FILE_NAME), accounts)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = config_dir;
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

    #[cfg(target_os = "macos")]
    #[test]
    fn the_file_keeps_the_order_and_only_the_user_can_read_it() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("mailcrew-credentials-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let accounts = vec![account("b@x"), account("a@x")];
        save_all(&dir, &accounts).unwrap();
        assert_eq!(load_all(&dir).unwrap(), accounts);
        let mode = std::fs::metadata(dir.join(FILE_NAME))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        // With every account gone the file stays, empty, so the Keychain is
        // not read again.
        save_all(&dir, &[]).unwrap();
        assert!(load_all(&dir).unwrap().is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_old_entry_reads_as_an_account() {
        let json = r#"{"clientId":"id","clientSecret":"s","refreshToken":"r","email":"a@x"}"#;
        let account: Account = serde_json::from_str(json).unwrap();
        assert_eq!(account.email, "a@x");
        assert_eq!(account.refresh_token, "r");
    }
}
