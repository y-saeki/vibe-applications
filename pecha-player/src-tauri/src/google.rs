//! Downloading a Google Slides deck as PDF.
//!
//! The deck is fetched from its `export/pdf` endpoint here rather than by the
//! webview, which cannot read a cross-origin response. Private decks need the
//! user's Google session: the sign-in window (`signin.rs`) leaves its cookies
//! in the webview's cookie store, and every request below carries the cookies
//! that store holds for the URL being requested, exactly as a browser would.
//!
//! What may be fetched is narrow on purpose. The URL the user pastes only
//! contributes a deck id; the request URL is rebuilt from it. Redirects are
//! followed by hand, and each hop has to stay on https and on Google's own
//! document hosts, so a deck id can never steer a request (or the cookies that
//! go with it) anywhere else.

use std::io::Read;
use std::time::Duration;

use serde::Serialize;
use tauri::Url;

/// Largest deck we accept. Google's own export of a 20-slide deck with photos
/// is a few tens of megabytes; this leaves room without letting a response
/// exhaust memory.
pub const MAX_PDF_BYTES: u64 = 300 * 1024 * 1024;

const MAX_REDIRECTS: usize = 8;
const TIMEOUT: Duration = Duration::from_secs(120);

/// Why a deck could not be loaded. Serialized as `{ "kind": "..." }` for the
/// frontend, which turns each kind into a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DeckError {
    /// The text is not a Google Slides link.
    InvalidUrl,
    /// Google asked for a sign-in before handing out the deck.
    SignInRequired,
    /// Signed in, but this account may not see the deck.
    Forbidden,
    /// No deck behind that id.
    NotFound,
    /// The deck is larger than `MAX_PDF_BYTES`.
    TooLarge,
    /// The response was not a PDF.
    NotPdf,
    /// Anything on the way: DNS, TLS, timeouts, unexpected statuses.
    Network { detail: String },
}

/// Pulls the deck id out of a link to a Google Slides deck.
///
/// Accepts the forms the browser's address bar and the Share dialog produce:
/// `https://docs.google.com/presentation/d/<id>/edit?...`, with an optional
/// `/u/<n>` account segment before `/d/`. Published (`/d/e/...`) links are not
/// decks one can export and are rejected.
pub fn parse_deck_id(input: &str) -> Result<String, DeckError> {
    let url = Url::parse(input.trim()).map_err(|_| DeckError::InvalidUrl)?;
    if url.scheme() != "https" || url.host_str() != Some("docs.google.com") {
        return Err(DeckError::InvalidUrl);
    }
    let mut segments = url.path_segments().ok_or(DeckError::InvalidUrl)?;
    if segments.next() != Some("presentation") {
        return Err(DeckError::InvalidUrl);
    }
    let mut next = segments.next();
    if next == Some("u") {
        // `/u/<n>/d/<id>`: skip the account index.
        segments.next();
        next = segments.next();
    }
    if next != Some("d") {
        return Err(DeckError::InvalidUrl);
    }
    let id = segments.next().ok_or(DeckError::InvalidUrl)?;
    if is_deck_id(id) {
        Ok(id.to_owned())
    } else {
        Err(DeckError::InvalidUrl)
    }
}

/// Deck ids are URL-safe base64-ish strings, 44 characters in practice. The
/// bounds are loose but keep anything odd out of the rebuilt URL.
fn is_deck_id(id: &str) -> bool {
    (20..=128).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn export_url(id: &str) -> Url {
    Url::parse(&format!(
        "https://docs.google.com/presentation/d/{id}/export/pdf"
    ))
    .expect("a checked deck id makes a valid URL")
}

/// The page the sign-in window opens: Google's sign-in, continuing to the deck
/// once it is done (straight away when the session is already there).
pub fn sign_in_url(id: &str) -> Url {
    let mut url = Url::parse("https://accounts.google.com/ServiceLogin").expect("constant URL");
    url.query_pairs_mut()
        .append_pair("continue", deck_url(id).as_str());
    url
}

/// The deck itself, which the sign-in window lands on once Google is satisfied.
pub fn deck_url(id: &str) -> Url {
    Url::parse(&format!("https://docs.google.com/presentation/d/{id}/edit"))
        .expect("a checked deck id makes a valid URL")
}

/// Whether the sign-in window has reached the deck, which means Google let the
/// session through.
pub fn is_deck_page(url: &Url, id: &str) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some("docs.google.com")
        && url.path().starts_with(&format!("/presentation/d/{id}"))
}

/// Where a redirect from the export endpoint may lead.
#[derive(Debug, PartialEq, Eq)]
enum Hop {
    /// Another Google document host: follow it.
    Follow(Url),
    /// Google's sign-in page: the session is missing.
    SignIn,
}

fn classify_redirect(from: &Url, location: &str) -> Result<Hop, DeckError> {
    let to = from.join(location).map_err(|_| network("bad redirect"))?;
    if to.scheme() != "https" {
        return Err(network("redirect left https"));
    }
    let host = to.host_str().unwrap_or_default();
    if host == "accounts.google.com" {
        return Ok(Hop::SignIn);
    }
    if host == "docs.google.com" || host.ends_with(".googleusercontent.com") {
        return Ok(Hop::Follow(to));
    }
    Err(network(&format!("unexpected redirect to {host}")))
}

/// PDF files start with `%PDF-`, which the spec allows within the first
/// kilobyte.
pub fn looks_like_pdf(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    head.windows(5).any(|w| w == b"%PDF-")
}

fn network(detail: &str) -> DeckError {
    DeckError::Network {
        detail: detail.to_owned(),
    }
}

/// Downloads deck `id` as PDF. `cookies` returns the `Cookie` header value for
/// a URL, or an empty string for none.
pub fn download(id: &str, cookies: impl Fn(&Url) -> String) -> Result<Vec<u8>, DeckError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .https_only(true)
        .http_status_as_error(false)
        .timeout_global(Some(TIMEOUT))
        .build()
        .into();

    let mut url = export_url(id);
    for _ in 0..=MAX_REDIRECTS {
        let mut request = agent.get(url.as_str());
        let cookie = cookies(&url);
        if !cookie.is_empty() {
            request = request.header("Cookie", &cookie);
        }
        let mut response = request.call().map_err(|e| network(&e.to_string()))?;
        let status = response.status().as_u16();
        match status {
            300..=399 => {
                let location = response
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| network("redirect without a location"))?;
                match classify_redirect(&url, location)? {
                    Hop::Follow(next) => url = next,
                    Hop::SignIn => return Err(DeckError::SignInRequired),
                }
            }
            200 => {
                if let Some(len) = response
                    .headers()
                    .get("content-length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                {
                    if len > MAX_PDF_BYTES {
                        return Err(DeckError::TooLarge);
                    }
                }
                let mut bytes = Vec::new();
                response
                    .body_mut()
                    .as_reader()
                    .take(MAX_PDF_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| network(&e.to_string()))?;
                if bytes.len() as u64 > MAX_PDF_BYTES {
                    return Err(DeckError::TooLarge);
                }
                if looks_like_pdf(&bytes) {
                    return Ok(bytes);
                }
                // An HTML page instead of the file is Google asking who we are
                // (or to request access) rather than handing it over.
                return Err(if cookie.is_empty() {
                    DeckError::SignInRequired
                } else {
                    DeckError::NotPdf
                });
            }
            401 => return Err(DeckError::SignInRequired),
            403 => {
                return Err(if cookie.is_empty() {
                    DeckError::SignInRequired
                } else {
                    DeckError::Forbidden
                })
            }
            404 => return Err(DeckError::NotFound),
            _ => return Err(network(&format!("HTTP {status}"))),
        }
    }
    Err(network("too many redirects"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "1AbCdEfGhIjKlMnOpQrStUvWxYz0123456789_-abcde";

    #[test]
    fn parses_the_links_the_browser_and_share_dialog_produce() {
        for link in [
            format!("https://docs.google.com/presentation/d/{ID}/edit"),
            format!("https://docs.google.com/presentation/d/{ID}/edit?usp=sharing"),
            format!("https://docs.google.com/presentation/d/{ID}/edit#slide=id.p"),
            format!("https://docs.google.com/presentation/d/{ID}"),
            format!("https://docs.google.com/presentation/u/1/d/{ID}/view"),
            format!("  https://docs.google.com/presentation/d/{ID}/present  "),
        ] {
            assert_eq!(parse_deck_id(&link), Ok(ID.to_owned()), "{link}");
        }
    }

    #[test]
    fn rejects_anything_that_is_not_a_slides_deck() {
        for link in [
            "",
            "not a url",
            &format!("http://docs.google.com/presentation/d/{ID}/edit"),
            &format!("https://docs.google.com.evil.example/presentation/d/{ID}/edit"),
            &format!("https://evil.example/presentation/d/{ID}/edit"),
            &format!("https://docs.google.com/document/d/{ID}/edit"),
            &format!("https://docs.google.com/presentation/d/e/{ID}/pub"),
            "https://docs.google.com/presentation/d/short/edit",
            "https://docs.google.com/presentation/d/has%2Fslash0123456789abcdef/edit",
            "file:///etc/passwd",
        ] {
            assert_eq!(parse_deck_id(link), Err(DeckError::InvalidUrl), "{link}");
        }
    }

    #[test]
    fn redirects_stay_on_google_document_hosts() {
        let from = export_url(ID);
        assert_eq!(
            classify_redirect(
                &from,
                "https://doc-0g-1c-docs.googleusercontent.com/export/x"
            ),
            Ok(Hop::Follow(
                Url::parse("https://doc-0g-1c-docs.googleusercontent.com/export/x").unwrap()
            ))
        );
        assert_eq!(
            classify_redirect(&from, "/presentation/d/x/export/pdf?id=1"),
            Ok(Hop::Follow(
                Url::parse("https://docs.google.com/presentation/d/x/export/pdf?id=1").unwrap()
            ))
        );
        assert_eq!(
            classify_redirect(&from, "https://accounts.google.com/ServiceLogin?x=1"),
            Ok(Hop::SignIn)
        );
        for bad in [
            "http://doc-0g-1c-docs.googleusercontent.com/x",
            "https://evil.example/x",
            "https://googleusercontent.com.evil.example/x",
            "file:///etc/passwd",
        ] {
            assert!(classify_redirect(&from, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn recognises_the_deck_page_only_for_its_own_id() {
        assert!(is_deck_page(&deck_url(ID), ID));
        assert!(!is_deck_page(&deck_url(ID), "another0123456789abcdef"));
        assert!(!is_deck_page(&sign_in_url(ID), ID));
    }

    #[test]
    fn sign_in_continues_to_the_deck() {
        let url = sign_in_url(ID);
        let continue_to = url
            .query_pairs()
            .find(|(k, _)| k == "continue")
            .map(|(_, v)| v.into_owned());
        assert_eq!(continue_to.as_deref(), Some(deck_url(ID).as_str()));
    }

    #[test]
    fn detects_pdf_by_its_header() {
        assert!(looks_like_pdf(b"%PDF-1.7\n..."));
        assert!(looks_like_pdf(b"\xef\xbb\xbf%PDF-1.4"));
        assert!(!looks_like_pdf(b"<!DOCTYPE html><html>"));
        assert!(!looks_like_pdf(b""));
    }
}
