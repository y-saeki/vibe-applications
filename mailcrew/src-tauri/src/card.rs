//! Turning a Gmail thread into what one card shows, and a message into the
//! body a card shows once it comes near the screen.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::attachments;
use crate::authres::Authentication;
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

/// Where a part's bytes are: sent along with the message, or to be fetched
/// with `messages.attachments.get`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PartSource {
    /// base64url, as Gmail sent it.
    Data(String),
    /// The attachment id, valid for the message as it was fetched.
    Fetch(String),
    #[default]
    Missing,
}

impl PartSource {
    fn of(part: &MessagePart) -> Self {
        match (&part.body.data, &part.body.attachment_id) {
            (Some(data), _) => Self::Data(data.clone()),
            (None, Some(id)) => Self::Fetch(id.clone()),
            (None, None) => Self::Missing,
        }
    }
}

/// A file attached to a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    /// The part's id within the message, which names it to the commands that
    /// open and save it.
    pub id: String,
    pub name: String,
    pub mime_type: String,
    /// Bytes.
    pub size: u64,
    /// Whether Mail Crew opens it (`attachments::is_openable`); otherwise it
    /// can only be saved.
    pub openable: bool,
    #[serde(skip)]
    pub source: PartSource,
}

/// An image the HTML body shows from inside the message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedImage {
    /// In lower case, as `cid:` URLs are compared.
    pub content_id: String,
    pub mime_type: String,
    pub size: u64,
    pub source: PartSource,
}

/// The images the message's HTML body shows from inside the message.
pub fn embedded_images(message: &Message) -> Vec<EmbeddedImage> {
    let payload = &message.payload;
    let mime::Body::Html(html) = payload.body() else {
        return Vec::new();
    };
    payload
        .embedded_images(&html)
        .into_iter()
        .filter_map(|part| {
            Some(EmbeddedImage {
                content_id: part.content_id()?.to_ascii_lowercase(),
                mime_type: part.mime_type.to_ascii_lowercase(),
                size: part.body.size,
                source: PartSource::of(part),
            })
        })
        .collect()
}

/// An image's bytes as a `data:` URL, for an image type and nothing else.
pub fn data_url(mime_type: &str, bytes: &[u8]) -> Option<String> {
    use base64::Engine as _;
    let subtype = mime_type.strip_prefix("image/")?;
    let plain = !subtype.is_empty()
        && subtype
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-'));
    plain.then(|| {
        format!(
            "data:{mime_type};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    })
}

/// The body of the message a card shows, fetched apart from the card: a full
/// message costs Gmail quota several times what its headers do, so it is
/// fetched only for the cards that come near the screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageContent {
    pub body: CardBody,
    pub attachments: Vec<Attachment>,
    /// What Gmail found when it checked the sender.
    pub authentication: Authentication,
    /// The author's address when the message names exactly one, which BIMI
    /// requires before it looks for a logo. The page has the address already.
    #[serde(skip)]
    pub author: Option<String>,
}

impl MessageContent {
    /// From a message fetched with `format=full`. `images` maps the content
    /// ids of the images its body shows from inside it, in lower case, to
    /// `data:` URLs of them.
    pub fn from_message(message: &Message, images: &HashMap<String, String>) -> Self {
        let payload = &message.payload;
        let mut embedded = Vec::new();
        let body = match payload.body() {
            mime::Body::Html(html) => {
                embedded = payload.embedded_images(&html);
                let clean = sanitize_html(&html, images);
                CardBody::Html {
                    html: clean.html,
                    remote_images: clean.remote_images,
                }
            }
            mime::Body::Text(text) => CardBody::Text { text },
            mime::Body::Empty => CardBody::Empty,
        };
        let attachments = payload
            .attachments(&embedded)
            .into_iter()
            .map(|part| {
                let name = part.file_name();
                Attachment {
                    id: part.part_id.clone(),
                    openable: attachments::is_openable(&name),
                    name,
                    mime_type: part.mime_type.clone(),
                    size: part.body.size,
                    source: PartSource::of(part),
                }
            })
            .collect();
        Self {
            body,
            attachments,
            authentication: Authentication::from_headers(headers(
                payload,
                "Authentication-Results",
            )),
            author: single_author(&headers(payload, "From").collect::<Vec<_>>()),
        }
    }
}

/// Every value of the headers called `name`, in the order they come.
fn headers<'a>(part: &'a MessagePart, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    part.headers
        .iter()
        .filter(move |header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str())
}

/// The address in the `From` header, when there is one such header holding
/// one address.
fn single_author(values: &[&str]) -> Option<String> {
    let [value] = values else { return None };
    // Display names may hold an `@` or a `,` of their own when quoted.
    let mut bare = String::new();
    let mut quoted = false;
    let mut depth = 0usize;
    for ch in value.chars() {
        match ch {
            '"' if depth == 0 => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted && depth > 0 => depth -= 1,
            _ if quoted || depth > 0 => {}
            _ => bare.push(ch),
        }
    }
    if bare.matches('@').count() != 1 {
        return None;
    }
    let email = mime::parse_address(value).email.to_ascii_lowercase();
    email.contains('@').then_some(email)
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
            MessageContent::from_message(&message, &HashMap::new()),
            MessageContent {
                body: CardBody::Html {
                    html: "<p>two</p>".into(),
                    remote_images: false
                },
                attachments: Vec::new(),
                authentication: Authentication::default(),
                author: Some("b@example.com".into()),
            }
        );
    }

    #[test]
    fn a_message_carries_gmails_checks_and_its_one_author() {
        let mut value = message(
            "m1",
            &["INBOX"],
            "1",
            "\"Shop, Inc. (sales@shop.example)\" <News@Shop.example>",
            "S",
            Some("<p>x</p>"),
        );
        let headers = value["payload"]["headers"].as_array_mut().unwrap();
        headers.insert(
            0,
            serde_json::json!({ "name": "Authentication-Results", "value": "mx.google.com; dmarc=pass header.from=shop.example" }),
        );
        headers.push(serde_json::json!({ "name": "Authentication-Results", "value": "mx.google.com; dmarc=fail" }));
        let message: Message = serde_json::from_value(value).unwrap();
        let content = MessageContent::from_message(&message, &HashMap::new());
        assert!(content.authentication.dmarc_passed());
        assert_eq!(content.author.as_deref(), Some("news@shop.example"));
    }

    #[test]
    fn several_authors_are_no_author() {
        assert_eq!(single_author(&["a@example.com, b@example.com"]), None);
        assert_eq!(single_author(&["a@example.com", "b@example.com"]), None);
        assert_eq!(single_author(&[]), None);
        assert_eq!(single_author(&["no address"]), None);
        assert_eq!(
            single_author(&["Someone (a@b) <c@d.example>"]).as_deref(),
            Some("c@d.example")
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
            attachments: vec![Attachment {
                id: "2".into(),
                name: "a.pdf".into(),
                mime_type: "application/pdf".into(),
                size: 1024,
                openable: true,
                source: PartSource::Fetch("ANGjdJ".into()),
            }],
            authentication: Authentication {
                spf: None,
                dkim: None,
                dmarc: Some(crate::authres::Check {
                    result: "pass".into(),
                    domain: "example.com".into(),
                }),
            },
            author: Some("a@example.com".into()),
        };
        assert_eq!(
            serde_json::to_value(content).unwrap(),
            serde_json::json!({
                "body": { "kind": "html", "html": "x", "remoteImages": true },
                "attachments": [{
                    "id": "2",
                    "name": "a.pdf",
                    "mimeType": "application/pdf",
                    "size": 1024,
                    "openable": true
                }],
                "authentication": {
                    "spf": null,
                    "dkim": null,
                    "dmarc": { "result": "pass", "domain": "example.com" }
                }
            })
        );
        assert_eq!(
            serde_json::to_value(CardBody::Empty).unwrap(),
            serde_json::json!({ "kind": "empty" })
        );
    }

    #[test]
    fn a_message_lists_its_attachments_and_embeds_its_images() {
        let html = r#"<p>hi</p><img src="cid:logo@shop.example">"#;
        let b64 = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        let message: Message = serde_json::from_value(serde_json::json!({
            "id": "m1",
            "payload": {
                "partId": "",
                "mimeType": "multipart/mixed",
                "headers": [{ "name": "From", "value": "a@example.com" }],
                "parts": [
                    {
                        "partId": "0",
                        "mimeType": "multipart/related",
                        "parts": [
                            {
                                "partId": "0.0",
                                "mimeType": "text/html",
                                "headers": [{ "name": "Content-Type", "value": "text/html; charset=utf-8" }],
                                "body": { "data": b64(html.as_bytes()), "size": html.len() }
                            },
                            {
                                "partId": "0.1",
                                "mimeType": "image/png",
                                "filename": "logo.png",
                                "headers": [{ "name": "Content-ID", "value": "<Logo@shop.example>" }],
                                "body": { "attachmentId": "IMG", "size": 300 }
                            }
                        ]
                    },
                    {
                        "partId": "1",
                        "mimeType": "application/x-msdownload",
                        "filename": "setup.exe",
                        "body": { "attachmentId": "EXE", "size": 5000 }
                    },
                    {
                        "partId": "2",
                        "mimeType": "text/plain",
                        "filename": "memo.txt",
                        "body": { "data": b64(b"memo"), "size": 4 }
                    }
                ]
            }
        }))
        .unwrap();
        assert_eq!(
            embedded_images(&message),
            vec![EmbeddedImage {
                content_id: "logo@shop.example".into(),
                mime_type: "image/png".into(),
                size: 300,
                source: PartSource::Fetch("IMG".into()),
            }]
        );
        let images = HashMap::from([(
            "logo@shop.example".to_string(),
            data_url("image/png", b"png").unwrap(),
        )]);
        let content = MessageContent::from_message(&message, &images);
        assert_eq!(
            content.body,
            CardBody::Html {
                html: r#"<p>hi</p><img src="data:image/png;base64,cG5n">"#.into(),
                remote_images: false
            }
        );
        assert_eq!(
            content.attachments,
            vec![
                Attachment {
                    id: "1".into(),
                    name: "setup.exe".into(),
                    mime_type: "application/x-msdownload".into(),
                    size: 5000,
                    openable: false,
                    source: PartSource::Fetch("EXE".into()),
                },
                Attachment {
                    id: "2".into(),
                    name: "memo.txt".into(),
                    mime_type: "text/plain".into(),
                    size: 4,
                    openable: true,
                    source: PartSource::Data(b64(b"memo")),
                },
            ]
        );
    }

    #[test]
    fn only_images_become_data_urls() {
        assert_eq!(
            data_url("image/gif", b"GIF").as_deref(),
            Some("data:image/gif;base64,R0lG")
        );
        assert!(data_url("image/svg+xml", b"x").is_some());
        assert_eq!(data_url("text/html", b"<script>"), None);
        assert_eq!(data_url("image/png;x=\"y\"", b"x"), None);
        assert_eq!(data_url("image/", b"x"), None);
    }

    #[test]
    fn decodes_snippet_entities() {
        assert_eq!(
            decode_entities("a &lt;b&gt; &#x41;&#66; &unknown; & c"),
            "a <b> AB &unknown; & c"
        );
    }
}
