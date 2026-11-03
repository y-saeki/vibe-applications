//! Turning a Gmail thread into what one card shows, and a message into the
//! body a card shows once it comes near the screen.

use serde::{Deserialize, Serialize};

use crate::mime::{self, MessagePart};
use crate::sanitize::sanitize_html;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Thread {
    pub id: String,
    pub messages: Vec<Message>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Message {
    pub id: String,
    pub label_ids: Vec<String>,
    /// Epoch milliseconds, as a string.
    pub internal_date: String,
    pub snippet: String,
    pub payload: MessagePart,
}

impl Message {
    fn has_label(&self, label: &str) -> bool {
        self.label_ids.iter().any(|id| id == label)
    }

    fn is_draft(&self) -> bool {
        self.has_label("DRAFT")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CardBody {
    Html { html: String, remote_images: bool },
    Text { text: String },
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: String,
    /// The address of the account the thread is in. Every action on the card
    /// goes to this account.
    pub account: String,
    pub subject: String,
    pub from: Person,
    /// Epoch milliseconds of the message shown.
    pub date: i64,
    pub message_count: usize,
    pub unread: bool,
    pub starred: bool,
    /// The message the card shows: the newest one that is not a draft.
    pub message_id: String,
    /// Every message in the thread but drafts. Archiving and tagging change
    /// these and only these, so a reply that arrives while the card is on
    /// screen stays in the inbox.
    pub message_ids: Vec<String>,
    pub snippet: String,
}

/// The body of the message a card shows, fetched apart from the card: a full
/// message costs Gmail quota several times what its headers do, so it is
/// fetched only for the cards that come near the screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageContent {
    pub body: CardBody,
    pub attachments: Vec<String>,
}

impl MessageContent {
    /// From a message fetched with `format=full`.
    pub fn from_message(message: &Message) -> Self {
        let payload = &message.payload;
        let body = match payload.body() {
            mime::Body::Html(html) => {
                let clean = sanitize_html(&html);
                CardBody::Html {
                    html: clean.html,
                    remote_images: clean.remote_images,
                }
            }
            mime::Body::Text(text) => CardBody::Text { text },
            mime::Body::Empty => CardBody::Empty,
        };
        Self {
            body,
            attachments: payload.attachments(),
        }
    }
}

impl Card {
    /// From a thread fetched with `format=metadata` (or `full`); only the
    /// headers, labels and dates are read. `None` for a thread with nothing in
    /// it but drafts.
    pub fn from_thread(thread: &Thread, account: &str) -> Option<Self> {
        let messages: Vec<&Message> = thread.messages.iter().filter(|m| !m.is_draft()).collect();
        let shown = *messages.last()?;
        let payload = &shown.payload;
        let header = |name: &str| {
            payload
                .header(name)
                .map(mime::decode_encoded_words)
                .unwrap_or_default()
        };
        let from = mime::parse_address(payload.header("From").unwrap_or_default());
        Some(Self {
            id: thread.id.clone(),
            account: account.to_string(),
            subject: header("Subject").trim().to_string(),
            from: Person {
                name: from.name,
                email: from.email,
            },
            date: shown.internal_date.parse().unwrap_or_default(),
            message_count: messages.len(),
            unread: messages.iter().any(|m| m.has_label("UNREAD")),
            starred: messages.iter().any(|m| m.has_label("STARRED")),
            message_id: shown.id.clone(),
            message_ids: messages.iter().map(|m| m.id.clone()).collect(),
            snippet: decode_entities(&shown.snippet),
        })
    }
}

/// Gmail's snippets come HTML-escaped (`&#39;`, `&amp;`); the page sets them as
/// text, so they are unescaped here.
fn decode_entities(snippet: &str) -> String {
    let mut out = String::with_capacity(snippet.len());
    let mut rest = snippet;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';').filter(|&end| end <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(ch) => {
                out.push(ch);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    fn message(
        id: &str,
        labels: &[&str],
        date: &str,
        from: &str,
        subject: &str,
        html: Option<&str>,
    ) -> serde_json::Value {
        let body = html.map(|html| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(html));
        serde_json::json!({
            "id": id,
            "labelIds": labels,
            "internalDate": date,
            "snippet": "it&#39;s &amp; snippet",
            "payload": {
                "mimeType": "text/html",
                "headers": [
                    { "name": "From", "value": from },
                    { "name": "Subject", "value": subject },
                    { "name": "Content-Type", "value": "text/html; charset=UTF-8" }
                ],
                "body": { "data": body }
            }
        })
    }

    fn thread(messages: Vec<serde_json::Value>) -> Thread {
        serde_json::from_value(serde_json::json!({ "id": "t1", "messages": messages })).unwrap()
    }

    #[test]
    fn shows_the_newest_message_that_is_not_a_draft() {
        let card = Card::from_thread(
            &thread(vec![
                message(
                    "m1",
                    &["INBOX", "STARRED"],
                    "1000",
                    "A <a@example.com>",
                    "First",
                    Some("<p>one</p>"),
                ),
                message(
                    "m2",
                    &["INBOX", "UNREAD"],
                    "2000",
                    "\"B\" <b@example.com>",
                    "Re: First",
                    Some("<p onclick=x>two</p>"),
                ),
                message(
                    "m3",
                    &["DRAFT"],
                    "3000",
                    "Me <me@example.com>",
                    "Re: First",
                    Some("<p>draft</p>"),
                ),
            ]),
            "me@example.com",
        )
        .unwrap();
        assert_eq!(card.id, "t1");
        assert_eq!(card.account, "me@example.com");
        assert_eq!(card.message_id, "m2");
        assert_eq!(card.message_ids, vec!["m1", "m2"]);
        assert_eq!(card.message_count, 2);
        assert_eq!(card.subject, "Re: First");
        assert_eq!(
            card.from,
            Person {
                name: "B".into(),
                email: "b@example.com".into()
            }
        );
        assert_eq!(card.date, 2000);
        assert!(card.unread);
        assert!(card.starred);
        assert_eq!(card.snippet, "it's & snippet");
    }

    #[test]
    fn a_message_body_is_sanitised() {
        let message: Message = serde_json::from_value(message(
            "m2",
            &["INBOX"],
            "2000",
            "b@example.com",
            "S",
            Some("<p onclick=x>two</p>"),
        ))
        .unwrap();
        assert_eq!(
            MessageContent::from_message(&message),
            MessageContent {
                body: CardBody::Html {
                    html: "<p>two</p>".into(),
                    remote_images: false
                },
                attachments: Vec::new(),
            }
        );
    }

    #[test]
    fn a_thread_of_drafts_has_no_card() {
        assert!(Card::from_thread(
            &thread(vec![message("m1", &["DRAFT"], "1", "x", "s", None)]),
            "me@example.com"
        )
        .is_none());
    }

    #[test]
    fn serialises_as_the_frontend_reads_it() {
        let card = Card::from_thread(
            &thread(vec![message(
                "m1",
                &["INBOX"],
                "5",
                "a@example.com",
                "S",
                None,
            )]),
            "me@example.com",
        )
        .unwrap();
        let json = serde_json::to_value(&card).unwrap();
        assert_eq!(json["account"], "me@example.com");
        assert!(json.get("body").is_none());
        assert_eq!(json["messageIds"], serde_json::json!(["m1"]));
        assert_eq!(json["from"]["email"], "a@example.com");
        let content = MessageContent {
            body: CardBody::Html {
                html: "x".into(),
                remote_images: true,
            },
            attachments: vec!["a.pdf".into()],
        };
        assert_eq!(
            serde_json::to_value(content).unwrap(),
            serde_json::json!({
                "body": { "kind": "html", "html": "x", "remoteImages": true },
                "attachments": ["a.pdf"]
            })
        );
        assert_eq!(
            serde_json::to_value(CardBody::Empty).unwrap(),
            serde_json::json!({ "kind": "empty" })
        );
    }

    #[test]
    fn decodes_snippet_entities() {
        assert_eq!(
            decode_entities("a &lt;b&gt; &#x41;&#66; &unknown; & c"),
            "a <b> AB &unknown; & c"
        );
    }
}
