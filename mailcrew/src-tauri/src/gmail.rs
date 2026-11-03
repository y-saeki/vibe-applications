//! The Gmail API, as far as mailcrew uses it: the inbox's threads, the user's
//! labels, and changing which labels messages carry. One `Gmail` speaks for one
//! signed-in account.

use std::collections::HashMap;
use std::time::Duration;

use futures::{StreamExt, TryStreamExt};
use serde::de::{DeserializeOwned, IgnoredAny};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::auth::{self, AccessToken};
use crate::card::{Card, Message, MessageContent, Thread};
use crate::credentials::Account;
use crate::error::{CommandError, ErrorKind, Result};

const API: &str = "https://gmail.googleapis.com/gmail/v1/users/me";

/// How many threads the inbox shows at once.
pub const PAGE_SIZE: usize = 50;
/// How many threads are fetched at the same time.
const FETCH_CONCURRENCY: usize = 8;
/// `messages.batchModify` takes at most this many ids per request.
const BATCH_LIMIT: usize = 1000;

/// How many times a request Google turned down for its rate limit is sent
/// again, waiting longer each time. The limit is counted per minute, so the
/// waits add up to a little over one.
const RATE_LIMIT_RETRIES: u32 = 6;
/// The longest wait between two tries.
const MAX_BACKOFF: Duration = Duration::from_secs(32);
/// How many threads' cards each account keeps between loads.
const CACHE_LIMIT: usize = 1000;
/// How many message bodies each account keeps.
const CONTENT_CACHE_LIMIT: usize = 200;

/// The label "あとで読む" puts on a thread.
pub const SET_ASIDE_LABEL: &str = "SetAside";

/// Which pile the strip shows: the inbox, or what "あとで読む" set aside.
/// Taking a card off the strip takes its thread out of that pile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mailbox {
    #[default]
    Inbox,
    SetAside,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: String,
    pub name: String,
}

/// A thread's card as last fetched, with the `historyId` it was fetched at.
/// Any change to the thread (a new message, a label) moves the id on.
struct Cached {
    history_id: String,
    /// `None` for a thread of drafts only.
    card: Option<Card>,
}

pub struct Gmail {
    http: reqwest::Client,
    account: Account,
    token: Mutex<Option<AccessToken>>,
    set_aside: Mutex<Option<String>>,
    /// Threads already fetched, so that a reload or another view asks Gmail
    /// only for the threads that changed.
    cache: Mutex<HashMap<String, Cached>>,
    /// Message bodies already fetched, by message id. A message's content
    /// never changes, only its labels.
    contents: Mutex<HashMap<String, MessageContent>>,
}

#[derive(Deserialize)]
struct ApiError {
    error: ApiErrorBody,
}

#[derive(Deserialize)]
struct ApiErrorBody {
    message: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    errors: Vec<ApiErrorItem>,
}

#[derive(Deserialize)]
struct ApiErrorItem {
    #[serde(default)]
    reason: String,
}

/// Whether Google turned a request down for the per-user or per-project rate
/// limit, which passes if the request waits and goes again.
pub fn is_rate_limited(status: u16, body: &str) -> bool {
    if status == 429 {
        return true;
    }
    if status != 403 {
        return false;
    }
    serde_json::from_str::<ApiError>(body).is_ok_and(|err| {
        err.error.status == "RESOURCE_EXHAUSTED"
            || err.error.errors.iter().any(|item| {
                matches!(
                    item.reason.as_str(),
                    "rateLimitExceeded" | "userRateLimitExceeded"
                )
            })
    })
}

/// How long to wait before the `attempt`-th retry (from 0): 1, 2, 4 … seconds
/// up to `MAX_BACKOFF`, with up to a second more so that parallel requests do
/// not all come back at once. This is the truncated exponential backoff the
/// Gmail API's usage limits page recommends.
fn backoff(attempt: u32) -> Duration {
    let base = Duration::from_secs(1 << attempt.min(6));
    let jitter = Duration::from_millis(rand::random_range(0..=1000));
    (base + jitter).min(MAX_BACKOFF)
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
            cache: Mutex::new(HashMap::new()),
            contents: Mutex::new(HashMap::new()),
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
    /// turns the current one down, and waiting and sending it again while
    /// Google says the rate limit is reached.
    async fn send<T: DeserializeOwned>(
        &self,
        build: impl Fn(&reqwest::Client) -> reqwest::RequestBuilder,
    ) -> Result<T> {
        let mut renew = false;
        let mut retries = 0;
        loop {
            let token = self.token(renew).await?;
            let response = build(&self.http).bearer_auth(token).send().await?;
            let status = response.status();
            if status == reqwest::StatusCode::UNAUTHORIZED && !renew {
                renew = true;
                continue;
            }
            let body = response.text().await?;
            if is_rate_limited(status.as_u16(), &body) {
                if retries < RATE_LIMIT_RETRIES {
                    tokio::time::sleep(backoff(retries)).await;
                    retries += 1;
                    continue;
                }
                return Err(CommandError::new(
                    ErrorKind::Google,
                    "Gmail の利用上限に達しました。1 分ほど待ってから再読み込みしてください",
                ));
            }
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
    pub async fn inbox(&self, mailbox: Mailbox, query: &str) -> Result<Vec<Card>> {
        #[derive(Deserialize)]
        struct ThreadList {
            #[serde(default)]
            threads: Vec<ThreadRef>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ThreadRef {
            id: String,
            #[serde(default)]
            history_id: String,
        }
        let label = match mailbox {
            Mailbox::Inbox => "INBOX".to_string(),
            // Nothing has been set aside until the label exists.
            Mailbox::SetAside => match self.find_set_aside_label().await? {
                Some(id) => id,
                None => return Ok(Vec::new()),
            },
        };
        let list: ThreadList = self
            .send(|http| {
                let request = http.get(format!("{API}/threads")).query(&[
                    ("labelIds", label.as_str()),
                    ("maxResults", &PAGE_SIZE.to_string()),
                ]);
                if query.trim().is_empty() {
                    request
                } else {
                    request.query(&[("q", query.trim())])
                }
            })
            .await?;
        // Only the threads that changed since they were last fetched are
        // fetched again; each full thread costs as much quota as the list.
        let stale: Vec<(String, String)> = {
            let cache = self.cache.lock().await;
            list.threads
                .iter()
                .filter(|thread| {
                    thread.history_id.is_empty()
                        || cache
                            .get(&thread.id)
                            .is_none_or(|cached| cached.history_id != thread.history_id)
                })
                .map(|thread| (thread.id.clone(), thread.history_id.clone()))
                .collect()
        };
        let fetched: Vec<(Thread, String)> = futures::stream::iter(stale)
            .map(|(id, history_id)| async move {
                let thread = self
                    .send::<Thread>(|http| {
                        // Headers and labels only: a full thread costs three
                        // times as much quota. The body comes with `message`.
                        http.get(format!("{API}/threads/{id}")).query(&[
                            ("format", "metadata"),
                            ("metadataHeaders", "From"),
                            ("metadataHeaders", "Subject"),
                        ])
                    })
                    .await?;
                Ok::<_, CommandError>((thread, history_id))
            })
            .buffered(FETCH_CONCURRENCY)
            .try_collect()
            .await?;
        let mut cache = self.cache.lock().await;
        for (thread, history_id) in fetched {
            let card = Card::from_thread(&thread, self.email());
            cache.insert(thread.id, Cached { history_id, card });
        }
        if cache.len() > CACHE_LIMIT {
            // Keeps what this page shows and lets the rest go.
            cache.retain(|id, _| list.threads.iter().any(|thread| &thread.id == id));
        }
        Ok(list
            .threads
            .iter()
            .filter_map(|thread| cache.get(&thread.id).and_then(|cached| cached.card.clone()))
            .collect())
    }

    /// The body and attachments of one message.
    pub async fn message(&self, id: &str) -> Result<MessageContent> {
        if let Some(content) = self.contents.lock().await.get(id) {
            return Ok(content.clone());
        }
        let message: Message = self
            .send(|http| {
                http.get(format!("{API}/messages/{id}"))
                    .query(&[("format", "full")])
            })
            .await?;
        let content = MessageContent::from_message(&message);
        let mut contents = self.contents.lock().await;
        if contents.len() >= CONTENT_CACHE_LIMIT {
            contents.clear();
        }
        contents.insert(id.to_string(), content.clone());
        Ok(content)
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

    /// The id of the "SetAside" label, without making it when there is none.
    async fn find_set_aside_label(&self) -> Result<Option<String>> {
        if let Some(id) = self.set_aside.lock().await.clone() {
            return Ok(Some(id));
        }
        let found = self
            .labels()
            .await?
            .into_iter()
            .find(|label| label.name.eq_ignore_ascii_case(SET_ASIDE_LABEL))
            .map(|label| label.id);
        if let Some(id) = &found {
            *self.set_aside.lock().await = Some(id.clone());
        }
        Ok(found)
    }

    /// The labels that keep a thread in `mailbox`, to take off when it leaves.
    /// `INBOX` goes either way, so that a set-aside thread that was moved back
    /// to the inbox in Gmail leaves both.
    pub async fn leaving(&self, mailbox: Mailbox) -> Result<Vec<String>> {
        Ok(match mailbox {
            Mailbox::Inbox => vec!["INBOX".to_string()],
            Mailbox::SetAside => vec!["INBOX".to_string(), self.set_aside_label().await?],
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_names_the_mailbox() {
        let inbox: Mailbox = serde_json::from_str(r#""inbox""#).unwrap();
        let set_aside: Mailbox = serde_json::from_str(r#""setAside""#).unwrap();
        assert_eq!(inbox, Mailbox::Inbox);
        assert_eq!(set_aside, Mailbox::SetAside);
        assert_eq!(Mailbox::default(), Mailbox::Inbox);
    }

    #[test]
    fn too_many_requests_is_a_rate_limit() {
        assert!(is_rate_limited(429, ""));
    }

    #[test]
    fn a_forbidden_rate_limit_is_one_too() {
        let body = r#"{"error":{"code":403,"message":"Quota exceeded","errors":[{"reason":"userRateLimitExceeded"}]}}"#;
        assert!(is_rate_limited(403, body));
        let exhausted =
            r#"{"error":{"code":403,"message":"Quota exceeded","status":"RESOURCE_EXHAUSTED"}}"#;
        assert!(is_rate_limited(403, exhausted));
    }

    #[test]
    fn other_errors_are_not() {
        let denied = r#"{"error":{"code":403,"message":"Insufficient Permission","errors":[{"reason":"insufficientPermissions"}]}}"#;
        assert!(!is_rate_limited(403, denied));
        assert!(!is_rate_limited(404, ""));
        assert!(!is_rate_limited(500, ""));
    }

    #[test]
    fn waits_longer_each_time_up_to_the_cap() {
        for attempt in 0..5 {
            let wait = backoff(attempt);
            let base = Duration::from_secs(1 << attempt);
            assert!(wait >= base && wait <= base + Duration::from_secs(1));
        }
        assert_eq!(backoff(5), MAX_BACKOFF);
        assert_eq!(backoff(9), MAX_BACKOFF);
        let total: Duration = (0..RATE_LIMIT_RETRIES).map(backoff).sum();
        assert!(total >= Duration::from_secs(60));
    }
}
