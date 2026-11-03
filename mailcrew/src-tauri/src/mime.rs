//! Reading a message out of the structure the Gmail API returns with
//! `format=full`: the headers, the body to show, and the attachments' names.
//! Bodies arrive as base64url bytes in whatever charset the sender used.

use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::Engine as _;
use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::Encoding;
use serde::Deserialize;

/// base64url, with or without padding: Gmail has been seen to send both.
const BASE64URL: GeneralPurpose = GeneralPurpose::new(
    &alphabet::URL_SAFE,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MessagePart {
    pub mime_type: String,
    pub filename: String,
    pub headers: Vec<Header>,
    pub body: PartBody,
    pub parts: Vec<MessagePart>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartBody {
    pub data: Option<String>,
    pub attachment_id: Option<String>,
}

/// The body a card shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    Html(String),
    Text(String),
    Empty,
}

impl MessagePart {
    /// The first header called `name`, compared case-insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case(name))
            .map(|header| header.value.as_str())
    }

    fn is_attachment(&self) -> bool {
        let disposition = self.header("Content-Disposition").unwrap_or_default();
        !self.filename.is_empty()
            || disposition
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("attachment")
    }

    /// The part's text, decoded from its charset.
    fn text(&self) -> Option<String> {
        let bytes = BASE64URL.decode(self.body.data.as_deref()?).ok()?;
        let charset = self
            .header("Content-Type")
            .and_then(|value| content_type_param(value, "charset"));
        Some(decode_text(&bytes, charset.as_deref()))
    }

    /// The HTML body if the message has one, else the plain text one.
    pub fn body(&self) -> Body {
        let mut html = None;
        let mut text = None;
        self.find_bodies(&mut html, &mut text);
        match (html, text) {
            (Some(html), _) => Body::Html(html),
            (None, Some(text)) => Body::Text(text),
            (None, None) => Body::Empty,
        }
    }

    fn find_bodies(&self, html: &mut Option<String>, text: &mut Option<String>) {
        if self.is_attachment() {
            return;
        }
        let mime = self.mime_type.to_ascii_lowercase();
        if mime.starts_with("multipart/") {
            for part in &self.parts {
                part.find_bodies(html, text);
            }
        } else if mime == "text/html" && html.is_none() {
            *html = self.text();
        } else if mime == "text/plain" && text.is_none() {
            *text = self.text();
        }
    }

    /// The file names of everything attached, in order.
    pub fn attachments(&self) -> Vec<String> {
        let mut names = Vec::new();
        self.collect_attachments(&mut names);
        names
    }

    fn collect_attachments(&self, names: &mut Vec<String>) {
        let disposition = self
            .header("Content-Disposition")
            .unwrap_or_default()
            .to_ascii_lowercase();
        // Images placed in the HTML body are parts of the body, not attachments.
        let inline =
            disposition.trim_start().starts_with("inline") && self.header("Content-ID").is_some();
        if !self.filename.is_empty() && !inline {
            names.push(decode_encoded_words(&self.filename));
        }
        for part in &self.parts {
            part.collect_attachments(names);
        }
    }
}

/// One parameter of a `Content-Type` value, unquoted.
pub fn content_type_param(value: &str, name: &str) -> Option<String> {
    value.split(';').skip(1).find_map(|param| {
        let (key, value) = param.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().trim_matches('"').to_string())
    })
}

/// The encoding a charset label names. encoding_rs knows the labels browsers
/// use; mail also carries a few it does not, which are mapped here.
fn encoding_for_label(label: &str) -> Option<&'static Encoding> {
    let label = label.trim().trim_matches('"').to_ascii_lowercase();
    // US-ASCII is a label browsers read as windows-1252, which decodes any
    // byte at all; mail labelled ASCII that holds anything else is mislabelled,
    // and treating the label as absent lets the bytes speak for themselves.
    if matches!(
        label.as_str(),
        "us-ascii" | "ascii" | "ansi_x3.4-1968" | "us"
    ) {
        return None;
    }
    if let Some(encoding) = Encoding::for_label(label.as_bytes()) {
        return Some(encoding);
    }
    match label.as_str() {
        // Microsoft's Shift_JIS, which encoding_rs's Shift_JIS already is.
        "cp932" | "x-ms-cp932" | "windows-932" | "ibm-943" => Some(encoding_rs::SHIFT_JIS),
        "eucjp" | "euc-jp-ms" | "x-euc-jp-ms" => Some(encoding_rs::EUC_JP),
        _ if label.starts_with("iso-2022-jp") => Some(encoding_rs::ISO_2022_JP),
        _ => None,
    }
}

/// The charset an HTML body declares for itself in a `<meta>` element, for a
/// part whose header names none.
fn meta_charset(bytes: &[u8]) -> Option<&'static Encoding> {
    let head = &bytes[..bytes.len().min(4096)];
    let lower = head.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = find(&lower[from..], b"charset") {
        let mut i = from + at + b"charset".len();
        from = i;
        while i < lower.len() && lower[i].is_ascii_whitespace() {
            i += 1;
        }
        if lower.get(i) != Some(&b'=') {
            continue;
        }
        i += 1;
        while i < lower.len()
            && (lower[i].is_ascii_whitespace() || lower[i] == b'"' || lower[i] == b'\'')
        {
            i += 1;
        }
        let end = lower[i..]
            .iter()
            .position(|b| !(b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':')))
            .map_or(lower.len(), |len| i + len);
        if let Some(encoding) = std::str::from_utf8(&lower[i..end])
            .ok()
            .and_then(encoding_for_label)
        {
            return Some(encoding);
        }
    }
    None
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Decodes without putting U+FFFD in anywhere, or not at all.
fn decode_cleanly(bytes: &[u8], encoding: &'static Encoding) -> Option<String> {
    encoding
        .decode_without_bom_handling_and_without_replacement(bytes)
        .map(|text| text.into_owned())
}

/// Bytes in a named charset, as text.
///
/// Mail often names its charset wrongly or not at all, and reading Japanese
/// bytes as UTF-8 turns every character into U+FFFD. So the label is only the
/// first guess: then a charset the HTML declares for itself, then ISO-2022-JP
/// if its escape sequences are there, then UTF-8, then
/// what chardetng (Firefox's detector) makes of the bytes. The first that
/// decodes without a single error wins; if none does, the label (or UTF-8)
/// decodes with replacement characters, as before.
pub fn decode_text(bytes: &[u8], charset: Option<&str>) -> String {
    let labelled = charset.and_then(encoding_for_label);
    // ISO-2022-JP is 7-bit, so it is valid UTF-8 too; its escape sequences
    // are what give it away.
    let iso_2022_jp = (bytes.is_ascii()
        && (find(bytes, b"\x1b$B").is_some() || find(bytes, b"\x1b$@").is_some()))
    .then_some(encoding_rs::ISO_2022_JP);
    let candidates = [
        labelled,
        meta_charset(bytes),
        iso_2022_jp,
        Some(encoding_rs::UTF_8),
        Some(guess(bytes)),
    ];
    for encoding in candidates.into_iter().flatten() {
        if let Some(text) = decode_cleanly(bytes, encoding) {
            return text;
        }
    }
    labelled
        .unwrap_or(encoding_rs::UTF_8)
        .decode(bytes)
        .0
        .into_owned()
}

fn guess(bytes: &[u8]) -> &'static Encoding {
    let mut detector = EncodingDetector::new(Iso2022JpDetection::Allow);
    detector.feed(bytes, true);
    detector.guess(None, Utf8Detection::Allow)
}

/// Decodes RFC 2047 encoded words (`=?ISO-2022-JP?B?...?=`) in a header value.
/// Whitespace between two adjacent encoded words is dropped, as the RFC says.
pub fn decode_encoded_words(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    let mut pending_space = String::new();
    let mut last_was_word = false;
    while !rest.is_empty() {
        if let Some((decoded, consumed)) = encoded_word_at(rest) {
            if !last_was_word {
                out.push_str(&pending_space);
            }
            pending_space.clear();
            out.push_str(&decoded);
            rest = &rest[consumed..];
            last_was_word = true;
            continue;
        }
        let ch = rest.chars().next().expect("rest is not empty");
        if ch.is_whitespace() {
            pending_space.push(ch);
        } else {
            out.push_str(&pending_space);
            pending_space.clear();
            out.push(ch);
            last_was_word = false;
        }
        rest = &rest[ch.len_utf8()..];
    }
    out.push_str(&pending_space);
    out
}

fn encoded_word_at(s: &str) -> Option<(String, usize)> {
    let inner = s.strip_prefix("=?")?;
    let (charset, inner) = inner.split_once('?')?;
    let (encoding, inner) = inner.split_once('?')?;
    let end = inner.find("?=")?;
    let text = &inner[..end];
    if charset.is_empty() || text.contains(char::is_whitespace) {
        return None;
    }
    // `charset*language` (RFC 2231) names the language too.
    let charset = charset.split('*').next().unwrap_or(charset);
    let bytes = match encoding.to_ascii_uppercase().as_str() {
        "B" => base64::engine::general_purpose::STANDARD
            .decode(text)
            .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(text))
            .ok()?,
        "Q" => decode_q(text),
        _ => return None,
    };
    let consumed = 2 + charset.len() + 1 + encoding.len() + 1 + end + 2;
    Some((decode_text(&bytes, Some(charset)), consumed))
}

fn decode_q(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'_' => out.push(b' '),
            b'=' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                    Some(byte) => {
                        out.push(byte);
                        i += 3;
                        continue;
                    }
                    None => out.push(b'='),
                }
            }
            byte => out.push(byte),
        }
        i += 1;
    }
    out
}

/// A sender, split into display name and address.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Address {
    pub name: String,
    pub email: String,
}

/// Reads the first mailbox out of a `From:` value such as
/// `"Name" <user@example.com>` or a bare `user@example.com`.
pub fn parse_address(value: &str) -> Address {
    let value = decode_encoded_words(value);
    if let (Some(open), Some(close)) = (value.rfind('<'), value.rfind('>')) {
        if open < close {
            let email = value[open + 1..close].trim().to_string();
            let name = value[..open]
                .trim()
                .trim_matches('"')
                .trim()
                .replace("\\\"", "\"");
            return Address { name, email };
        }
    }
    Address {
        name: String::new(),
        email: value.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    fn leaf(mime: &str, content_type: &str, data: &[u8]) -> MessagePart {
        MessagePart {
            mime_type: mime.into(),
            headers: vec![Header {
                name: "Content-Type".into(),
                value: content_type.into(),
            }],
            body: PartBody {
                data: Some(b64(data)),
                attachment_id: None,
            },
            ..Default::default()
        }
    }

    fn multipart(mime: &str, parts: Vec<MessagePart>) -> MessagePart {
        MessagePart {
            mime_type: mime.into(),
            parts,
            ..Default::default()
        }
    }

    #[test]
    fn prefers_html_in_alternative() {
        let message = multipart(
            "multipart/alternative",
            vec![
                leaf("text/plain", "text/plain; charset=utf-8", b"plain"),
                leaf("text/html", "text/html; charset=utf-8", b"<p>html</p>"),
            ],
        );
        assert_eq!(message.body(), Body::Html("<p>html</p>".into()));
    }

    #[test]
    fn falls_back_to_plain_text() {
        let message = multipart(
            "multipart/mixed",
            vec![leaf(
                "text/plain",
                "text/plain; charset=\"UTF-8\"",
                "こんにちは".as_bytes(),
            )],
        );
        assert_eq!(message.body(), Body::Text("こんにちは".into()));
    }

    #[test]
    fn decodes_legacy_japanese_charsets() {
        let (iso, _, _) = encoding_rs::ISO_2022_JP.encode("日本語のメール");
        let part = leaf("text/plain", "text/plain; charset=ISO-2022-JP", &iso);
        assert_eq!(part.body(), Body::Text("日本語のメール".into()));
        let (sjis, _, _) = encoding_rs::SHIFT_JIS.encode("シフトJIS");
        let part = leaf("text/plain", "text/plain; charset=Shift_JIS", &sjis);
        assert_eq!(part.body(), Body::Text("シフトJIS".into()));
    }

    #[test]
    fn skips_attached_text_and_lists_attachments() {
        let mut attached = leaf("text/plain", "text/plain", b"attached");
        attached.filename = "notes.txt".into();
        let mut inline_image = leaf("image/png", "image/png", b"png");
        inline_image.filename = "logo.png".into();
        inline_image.headers.push(Header {
            name: "Content-Disposition".into(),
            value: "inline; filename=logo.png".into(),
        });
        inline_image.headers.push(Header {
            name: "Content-ID".into(),
            value: "<logo>".into(),
        });
        let message = multipart(
            "multipart/mixed",
            vec![
                attached,
                leaf("text/plain", "text/plain", b"body"),
                inline_image,
            ],
        );
        assert_eq!(message.body(), Body::Text("body".into()));
        assert_eq!(message.attachments(), vec!["notes.txt".to_string()]);
    }

    #[test]
    fn reads_charsets_mail_names_that_browsers_do_not() {
        let (sjis, _, _) = encoding_rs::SHIFT_JIS.encode("ご利用ありがとうございます");
        assert_eq!(
            decode_text(&sjis, Some("CP932")),
            "ご利用ありがとうございます"
        );
        let (iso, _, _) = encoding_rs::ISO_2022_JP.encode("お知らせ");
        assert_eq!(decode_text(&iso, Some("ISO-2022-JP-MS")), "お知らせ");
    }

    #[test]
    fn falls_back_to_the_charset_the_html_declares() {
        let (sjis, _, _) = encoding_rs::SHIFT_JIS.encode(
            "<html><head><meta http-equiv=\"Content-Type\" content=\"text/html; charset=Shift_JIS\"></head><body>残高500円分を当てよう</body></html>",
        );
        let part = leaf("text/html", "text/html", &sjis);
        let Body::Html(html) = part.body() else {
            panic!("not html")
        };
        assert!(html.contains("残高500円分を当てよう"), "{html}");
        let (euc, _, _) = encoding_rs::EUC_JP.encode("<meta charset='euc-jp'><p>日経電子版</p>");
        assert!(decode_text(&euc, None).contains("日経電子版"));
    }

    #[test]
    fn detects_japanese_that_names_no_charset_or_the_wrong_one() {
        let text = "日経電子版の解約につきまして。ご契約内容をご確認ください。";
        let (sjis, _, _) = encoding_rs::SHIFT_JIS.encode(text);
        assert_eq!(decode_text(&sjis, None), text);
        assert_eq!(decode_text(&sjis, Some("utf-8")), text);
        let (euc, _, _) = encoding_rs::EUC_JP.encode(text);
        assert_eq!(decode_text(&euc, Some("us-ascii")), text);
        let (iso, _, _) = encoding_rs::ISO_2022_JP.encode(text);
        assert_eq!(decode_text(&iso, None), text);
    }

    #[test]
    fn a_correct_label_is_taken_as_it_is() {
        assert_eq!(decode_text("café".as_bytes(), Some("utf-8")), "café");
        let (latin, _, _) = encoding_rs::WINDOWS_1252.encode("café");
        assert_eq!(decode_text(&latin, Some("iso-8859-1")), "café");
    }

    #[test]
    fn empty_message_has_empty_body() {
        assert_eq!(multipart("multipart/mixed", vec![]).body(), Body::Empty);
    }

    #[test]
    fn accepts_padded_base64url() {
        let mut part = leaf("text/plain", "text/plain", b"");
        part.body.data = Some("YWI=".into());
        assert_eq!(part.body(), Body::Text("ab".into()));
    }

    #[test]
    fn decodes_encoded_words() {
        assert_eq!(
            decode_encoded_words("=?UTF-8?B?44GT44KT44Gr44Gh44Gv?="),
            "こんにちは"
        );
        assert_eq!(
            decode_encoded_words("=?ISO-2022-JP?B?GyRCRnxLXDhsGyhC?="),
            "日本語"
        );
        assert_eq!(
            decode_encoded_words("=?utf-8?Q?caf=C3=A9_au_lait?="),
            "café au lait"
        );
        // Whitespace between two encoded words goes; around plain text it stays.
        assert_eq!(
            decode_encoded_words("=?UTF-8?B?44GC?= =?UTF-8?B?44GE?="),
            "あい"
        );
        assert_eq!(
            decode_encoded_words("Re: =?UTF-8?B?44GC?= end"),
            "Re: あ end"
        );
        assert_eq!(decode_encoded_words("plain subject"), "plain subject");
        assert_eq!(decode_encoded_words("=?broken"), "=?broken");
    }

    #[test]
    fn parses_addresses() {
        assert_eq!(
            parse_address("\"Taro Yamada\" <taro@example.com>"),
            Address {
                name: "Taro Yamada".into(),
                email: "taro@example.com".into()
            }
        );
        assert_eq!(
            parse_address("=?UTF-8?B?5bGx55Sw?= <yamada@example.com>"),
            Address {
                name: "山田".into(),
                email: "yamada@example.com".into()
            }
        );
        assert_eq!(
            parse_address("solo@example.com"),
            Address {
                name: String::new(),
                email: "solo@example.com".into()
            }
        );
    }

    #[test]
    fn reads_content_type_params() {
        assert_eq!(
            content_type_param(
                "text/html; charset=\"iso-2022-jp\"; format=flowed",
                "charset"
            ),
            Some("iso-2022-jp".into())
        );
        assert_eq!(content_type_param("text/html", "charset"), None);
    }
}
