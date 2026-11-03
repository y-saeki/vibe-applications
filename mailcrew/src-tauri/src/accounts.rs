//! Every signed-in account, each with its own `Gmail`, and the inbox they make
//! up together.

use std::sync::Arc;

use serde::Serialize;
use tokio::sync::Mutex;

use crate::auth::AccessToken;
use crate::card::Card;
use crate::credentials::{self, Account};
use crate::error::{CommandError, ErrorKind, Result};
use crate::gmail::{self, Gmail, PAGE_SIZE};

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))?
}

/// An account whose part of the inbox could not be fetched.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountFailure {
    pub account: String,
    pub kind: ErrorKind,
    pub message: String,
}

/// One page of the inbox, across the accounts asked for.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxPage {
    pub cards: Vec<Card>,
    /// The accounts that did not answer. The others' cards are there all the same.
    pub failures: Vec<AccountFailure>,
}

pub struct Accounts {
    http: reqwest::Client,
    /// `None` until the credential store has been read.
    list: Mutex<Option<Vec<Arc<Gmail>>>>,
}

impl Accounts {
    pub fn new() -> Self {
        Self {
            http: gmail::http_client(),
            list: Mutex::new(None),
        }
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Every signed-in account, in the order they were added.
    pub async fn all(&self) -> Result<Vec<Arc<Gmail>>> {
        let mut list = self.list.lock().await;
        if list.is_none() {
            let stored = blocking(credentials::load_all).await?;
            let http = self.http.clone();
            *list = Some(
                stored
                    .into_iter()
                    .map(|account| Arc::new(Gmail::new(http.clone(), account, None)))
                    .collect(),
            );
        }
        Ok(list.clone().unwrap_or_default())
    }

    pub async fn emails(&self) -> Result<Vec<String>> {
        Ok(self
            .all()
            .await?
            .iter()
            .map(|gmail| gmail.email().to_string())
            .collect())
    }

    /// The account with this address.
    pub async fn get(&self, email: &str) -> Result<Arc<Gmail>> {
        self.all()
            .await?
            .into_iter()
            .find(|gmail| gmail.email() == email)
            .ok_or_else(|| {
                CommandError::new(
                    ErrorKind::SignedOut,
                    format!("{email} にはログインしていません"),
                )
            })
    }

    /// The client of the account added first, for adding another with it.
    pub async fn first_client(&self) -> Result<Option<(String, String)>> {
        Ok(self.all().await?.first().map(|gmail| {
            let account = gmail.account();
            (account.client_id.clone(), account.client_secret.clone())
        }))
    }

    /// Stores `account` and starts using it, in the place of one with the
    /// same address if there is one.
    pub async fn add(&self, account: Account, token: AccessToken) -> Result<()> {
        self.all().await?;
        let stored = account.clone();
        blocking(move || credentials::store(&stored)).await?;
        let gmail = Arc::new(Gmail::new(self.http.clone(), account, Some(token)));
        let mut list = self.list.lock().await;
        let list = list.get_or_insert_with(Vec::new);
        match list.iter().position(|known| known.email() == gmail.email()) {
            Some(at) => list[at] = gmail,
            None => list.push(gmail),
        }
        Ok(())
    }

    /// Stops using the account with this address and forgets it.
    pub async fn remove(&self, email: &str) -> Result<()> {
        self.all().await?;
        if let Some(list) = self.list.lock().await.as_mut() {
            list.retain(|gmail| gmail.email() != email);
        }
        let email = email.to_string();
        blocking(move || credentials::remove(&email)).await
    }

    /// Forgets the account when `result` says Google no longer accepts it;
    /// keeping it would only fail again.
    pub async fn check<T>(&self, email: &str, result: Result<T>) -> Result<T> {
        if let Err(err) = &result {
            if err.kind == ErrorKind::SignedOut {
                let _ = self.remove(email).await;
            }
        }
        result
    }

    /// The newest `PAGE_SIZE` threads across `emails` (every account when it
    /// is empty), newest first.
    pub async fn inbox(&self, emails: &[String], query: &str) -> Result<InboxPage> {
        let all = self.all().await?;
        if all.is_empty() {
            return Err(CommandError::signed_out());
        }
        let chosen: Vec<Arc<Gmail>> = all
            .into_iter()
            .filter(|gmail| emails.is_empty() || emails.iter().any(|email| email == gmail.email()))
            .collect();
        let results =
            futures::future::join_all(chosen.iter().map(|gmail| gmail.inbox(query))).await;
        let mut pages = Vec::new();
        let mut failures = Vec::new();
        for (gmail, result) in chosen.iter().zip(results) {
            match self.check(gmail.email(), result).await {
                Ok(cards) => pages.push(cards),
                Err(err) => failures.push(AccountFailure {
                    account: gmail.email().to_string(),
                    kind: err.kind,
                    message: err.message,
                }),
            }
        }
        Ok(InboxPage {
            cards: newest(pages, PAGE_SIZE),
            failures,
        })
    }
}

/// The newest `limit` cards of several accounts' pages, newest first. Each page
/// holds its account's newest threads, so whatever belongs in the first
/// `limit` overall is in one of them.
pub fn newest(pages: Vec<Vec<Card>>, limit: usize) -> Vec<Card> {
    let mut cards: Vec<Card> = pages.into_iter().flatten().collect();
    // Stable, so threads of the same moment keep each account's own order.
    cards.sort_by_key(|card| std::cmp::Reverse(card.date));
    cards.truncate(limit);
    cards
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Person;

    fn card(id: &str, account: &str, date: i64) -> Card {
        Card {
            id: id.into(),
            account: account.into(),
            subject: String::new(),
            from: Person {
                name: String::new(),
                email: String::new(),
            },
            date,
            message_count: 1,
            unread: false,
            starred: false,
            message_id: id.into(),
            message_ids: vec![id.into()],
            snippet: String::new(),
        }
    }

    fn ids(cards: &[Card]) -> Vec<&str> {
        cards.iter().map(|card| card.id.as_str()).collect()
    }

    #[test]
    fn interleaves_accounts_by_date() {
        let merged = newest(
            vec![
                vec![card("a1", "a", 50), card("a2", "a", 20)],
                vec![card("b1", "b", 40), card("b2", "b", 10)],
            ],
            10,
        );
        assert_eq!(ids(&merged), vec!["a1", "b1", "a2", "b2"]);
    }

    #[test]
    fn keeps_only_the_newest_page() {
        let merged = newest(
            vec![
                vec![card("a1", "a", 50), card("a2", "a", 40)],
                vec![card("b1", "b", 45), card("b2", "b", 10)],
            ],
            3,
        );
        assert_eq!(ids(&merged), vec!["a1", "b1", "a2"]);
    }

    #[test]
    fn a_tie_keeps_the_order_it_came_in() {
        let merged = newest(vec![vec![card("a1", "a", 5)], vec![card("b1", "b", 5)]], 10);
        assert_eq!(ids(&merged), vec!["a1", "b1"]);
    }
}
