//! The Gmail API, as far as mailcrew uses it: the inbox's threads, the user's
//! labels, and changing which labels messages carry. One `Gmail` speaks for one
//! signed-in account.

use std::time::Duration;

use futures::{StreamExt, TryStreamExt};
use serde::de::{DeserializeOwned, IgnoredAny};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::auth::{self, AccessToken};
use crate::card::{Card, Thread};
use crate::credentials::Account;
use crate::error::{CommandError, ErrorKind, Result};

const API: &str = "https://gmail.googleapis.com/gmail/v1/users/me";

/// How many threads the inbox shows at once.
pub const PAGE_SIZE: usize = 50;
/// How many threads are fetched at the same time.
const FETCH_CONCURRENCY: usize = 8;
/// `messages.batchModify` takes at most this many ids per request.
const BATCH_LIMIT: usize = 1000;

/// The label "あとで読む" puts on a thread.
pub const SET_ASIDE_LABEL: &str = "SetAside";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: String,
    pub name: String,
}

pub struct Gmail {
    http: reqwest::Client,
    account: Account,
    token: Mutex<Option<AccessToken>>,
    set_aside: Mutex<Option<String>>,
}

#[derive(Deserialize)]
struct ApiError {
    error: ApiErrorBody,
}

#[derive(Deserialize)]
struct ApiErrorBody {
    message: String,
}

/// The one HTTP client every account's requests go through.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()
        .expect("the HTTP client builds with the default TLS backend")
}

impl Gmail {
    pub fn new(http: reqwest::Client, account: Account, token: Option<AccessToken>) -> Self {
        Self {
            http,
            account,
            token: Mutex::new(token),
            set_aside: Mutex::new(None),
        }
    }

    pub fn account(&self) -> &Account {
        &self.account
    }

    pub fn email(&self) -> &str {
        &self.account.email
    }

    /// An access token, renewed from the refresh token when it has run out or
    /// when `renew` asks for a new one regardless.
    async fn token(&self, renew: bool) -> Result<String> {
        let mut token = self.token.lock().await;
        if !renew {
            if let Some(token) = token.as_ref().filter(|token| token.is_fresh()) {
                return Ok(token.value.clone());
            }
        }
        let account = &self.account;
        let response = auth::refresh(
            &self.http,
            &account.client_id,
            &account.client_secret,
            &account.refresh_token,
        )
        .await
        .map_err(|err| match err.kind {
            ErrorKind::SignedOut if !account.email.is_empty() => CommandError::new(
                ErrorKind::SignedOut,
                format!(
                    "{} のログインが切れました。もう一度ログインしてください",
                    account.email
                ),
            ),
            _ => err,
        })?;
        let fresh = AccessToken::from_response(&response);
        let value = fresh.value.clone();
        *token = Some(fresh);
        Ok(value)
    }

    /// Sends a request built by `build`, renewing the token once if Google
    /// turns the current one down.
    async fn send<T: DeserializeOwned>(
        &self,
        build: impl Fn(&reqwest::Client) -> reqwest::RequestBuilder,
    ) -> Result<T> {
        let mut renew = false;
        loop {
            let token = self.token(renew).await?;
            let response = build(&self.http).bearer_auth(token).send().await?;
            let status = response.status();
            if status == reqwest::StatusCode::UNAUTHORIZED && !renew {
                renew = true;
                continue;
            }
            let body = response.text().await?;
            if !status.is_success() {
                let detail = serde_json::from_str::<ApiError>(&body)
                    .map(|err| err.error.message)
                    .unwrap_or_else(|_| status.to_string());
                return Err(CommandError::new(
                    ErrorKind::Google,
                    format!("Gmail がエラーを返しました: {detail}"),
                ));
            }
            let body = if body.trim().is_empty() {
                "null"
            } else {
                body.as_str()
            };
            return serde_json::from_str(body).map_err(|err| {
                CommandError::new(
                    ErrorKind::Google,
                    format!("Gmail の応答を読み取れませんでした: {err}"),
                )
            });
        }
    }

    pub async fn email_address(&self) -> Result<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Profile {
            email_address: String,
        }
        let profile: Profile = self.send(|http| http.get(format!("{API}/profile"))).await?;
        Ok(profile.email_address)
    }

    /// The newest `PAGE_SIZE` threads in the inbox, newest first, narrowed
    /// by a Gmail search (`from:`, `is:unread` and the rest) when `query` is
    /// not empty.
    pub async fn inbox(&self, query: &str) -> Result<Vec<Card>> {
        #[derive(Deserialize)]
        struct ThreadList {
            #[serde(default)]
            threads: Vec<ThreadRef>,
        }
        #[derive(Deserialize)]
        struct ThreadRef {
            id: String,
        }
        let list: ThreadList = self
            .send(|http| {
                let request = http.get(format!("{API}/threads")).query(&[
                    ("labelIds", "INBOX"),
                    ("maxResults", &PAGE_SIZE.to_string()),
                ]);
                if query.trim().is_empty() {
                    request
                } else {
                    request.query(&[("q", query.trim())])
                }
            })
            .await?;
        let threads: Vec<Thread> = futures::stream::iter(list.threads)
            .map(|thread| async move {
                self.send::<Thread>(|http| {
                    http.get(format!("{API}/threads/{}", thread.id))
                        .query(&[("format", "full")])
                })
                .await
            })
            .buffered(FETCH_CONCURRENCY)
            .try_collect()
            .await?;
        Ok(threads
            .iter()
            .filter_map(|thread| Card::from_thread(thread, self.email()))
            .collect())
    }

    /// The labels the user made, by name.
    pub async fn labels(&self) -> Result<Vec<Label>> {
        #[derive(Deserialize)]
        struct LabelList {
            #[serde(default)]
            labels: Vec<RawLabel>,
        }
        #[derive(Deserialize)]
        struct RawLabel {
            id: String,
            name: String,
            #[serde(rename = "type", default)]
            kind: String,
        }
        let list: LabelList = self.send(|http| http.get(format!("{API}/labels"))).await?;
        let mut labels: Vec<Label> = list
            .labels
            .into_iter()
            .filter(|label| label.kind == "user")
            .map(|label| Label {
                id: label.id,
                name: label.name,
            })
            .collect();
        labels.sort_by_key(|label| label.name.to_lowercase());
        Ok(labels)
    }

    /// Makes a label, or finds the one that already has that name.
    pub async fn create_label(&self, name: &str) -> Result<Label> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CommandError::new(ErrorKind::Internal, "タグの名前が空です"));
        }
        if let Some(existing) = self
            .labels()
            .await?
            .into_iter()
            .find(|label| label.name.eq_ignore_ascii_case(name))
        {
            return Ok(existing);
        }
        let body = serde_json::json!({
            "name": name,
            "labelListVisibility": "labelShow",
            "messageListVisibility": "show",
        });
        self.send(|http| http.post(format!("{API}/labels")).json(&body))
            .await
    }

    /// The id of the "SetAside" label, made the first time it is needed.
    pub async fn set_aside_label(&self) -> Result<String> {
        if let Some(id) = self.set_aside.lock().await.clone() {
            return Ok(id);
        }
        let id = self.create_label(SET_ASIDE_LABEL).await?.id;
        *self.set_aside.lock().await = Some(id.clone());
        Ok(id)
    }

    /// Adds and removes labels on the given messages.
    pub async fn modify(&self, ids: &[String], add: &[&str], remove: &[&str]) -> Result<()> {
        for chunk in ids.chunks(BATCH_LIMIT) {
            let body = serde_json::json!({
                "ids": chunk,
                "addLabelIds": add,
                "removeLabelIds": remove,
            });
            let _: IgnoredAny = self
                .send(|http| http.post(format!("{API}/messages/batchModify")).json(&body))
                .await?;
        }
        Ok(())
    }
}
