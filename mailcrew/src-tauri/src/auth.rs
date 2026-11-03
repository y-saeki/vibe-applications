//! Signing in to Google: OAuth 2.0 for installed apps with a loopback
//! redirect and PKCE. The browser comes back to a one-shot listener on
//! 127.0.0.1, which takes the authorization code and closes.

use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

use crate::error::{CommandError, ErrorKind, Result};

pub const AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
/// Reading messages, and changing their labels: archive, star, tag.
pub const SCOPE: &str = "https://www.googleapis.com/auth/gmail.modify";

/// How long the browser has to come back before the sign-in gives up.
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);

/// An access token is renewed this long before Google says it expires, so a
/// request never leaves with one that runs out on the way.
const EXPIRY_MARGIN: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn new() -> Self {
        Self::from_verifier(URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>()))
    }

    /// The S256 challenge for a given verifier (RFC 7636, section 4.2).
    pub fn from_verifier(verifier: String) -> Self {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
        }
    }
}

/// An unguessable value the redirect has to carry back.
pub fn random_state() -> String {
    URL_SAFE_NO_PAD.encode(rand::random::<[u8; 16]>())
}

pub fn authorization_url(
    client_id: &str,
    redirect_uri: &str,
    challenge: &str,
    state: &str,
) -> String {
    let mut url = Url::parse(AUTH_ENDPOINT).expect("the endpoint is a valid URL");
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPE)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state)
        // A refresh token, issued again on every consent, so that signing in
        // a second time replaces a lost one rather than coming back without.
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");
    url.into()
}

/// What one request to the loopback listener amounts to.
#[derive(Debug, PartialEq, Eq)]
pub enum Redirect {
    /// Google sent the user back with a code.
    Code(String),
    /// Google sent the user back without one (they declined, say).
    Denied(String),
    /// Not the redirect: a favicon request, or a state that does not match.
    Ignore,
}

/// Reads the request line of an HTTP request to the loopback listener.
pub fn parse_redirect(request_line: &str, expected_state: &str) -> Redirect {
    let mut words = request_line.split_whitespace();
    let (Some("GET"), Some(target)) = (words.next(), words.next()) else {
        return Redirect::Ignore;
    };
    let Ok(url) = Url::parse(&format!("http://127.0.0.1{target}")) else {
        return Redirect::Ignore;
    };
    if url.path() != "/" {
        return Redirect::Ignore;
    }
    let param = |name: &str| {
        url.query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
    };
    if param("state").as_deref() != Some(expected_state) {
        return Redirect::Ignore;
    }
    if let Some(code) = param("code") {
        return Redirect::Code(code);
    }
    Redirect::Denied(param("error").unwrap_or_else(|| "unknown".into()))
}

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>mailcrew</title>\
<p style=\"font: 15px system-ui; margin: 3em\">ログインが完了しました。このタブを閉じて mailcrew に戻ってください。</p>";
const DENIED_PAGE: &str = "<!doctype html><meta charset=utf-8><title>mailcrew</title>\
<p style=\"font: 15px system-ui; margin: 3em\">ログインできませんでした。mailcrew に戻ってやり直してください。</p>";

/// Answers requests on `listener` until one of them is the redirect.
pub async fn wait_for_code(listener: TcpListener, state: &str) -> Result<String> {
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|err| CommandError::new(ErrorKind::Internal, err.to_string()))?;
        let mut buffer = vec![0u8; 8192];
        let read = tokio::time::timeout(Duration::from_secs(10), stream.read(&mut buffer)).await;
        let Ok(Ok(read)) = read else { continue };
        let request = String::from_utf8_lossy(&buffer[..read]);
        let line = request.lines().next().unwrap_or_default();
        let (status, page, outcome) = match parse_redirect(line, state) {
            Redirect::Code(code) => ("200 OK", DONE_PAGE, Some(Ok(code))),
            Redirect::Denied(reason) => (
                "200 OK",
                DENIED_PAGE,
                Some(Err(CommandError::new(
                    ErrorKind::Google,
                    format!("Google がログインを許可しませんでした ({reason})"),
                ))),
            ),
            Redirect::Ignore => ("404 Not Found", "", None),
        };
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
            page.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;
        if let Some(outcome) = outcome {
            return outcome;
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: u64,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenError {
    error: String,
    error_description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AccessToken {
    pub value: String,
    pub renew_at: Instant,
}

impl AccessToken {
    pub fn from_response(response: &TokenResponse) -> Self {
        let lifetime = Duration::from_secs(response.expires_in).saturating_sub(EXPIRY_MARGIN);
        Self {
            value: response.access_token.clone(),
            renew_at: Instant::now() + lifetime,
        }
    }

    pub fn is_fresh(&self) -> bool {
        Instant::now() < self.renew_at
    }
}

async fn token_request(http: &reqwest::Client, form: &[(&str, &str)]) -> Result<TokenResponse> {
    let response = http.post(TOKEN_ENDPOINT).form(form).send().await?;
    if response.status().is_success() {
        return Ok(response.json().await?);
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let error: Option<TokenError> = serde_json::from_str(&body).ok();
    match error {
        // The refresh token was revoked, or expired: only a new sign-in helps.
        Some(error) if error.error == "invalid_grant" => Err(CommandError::new(
            ErrorKind::SignedOut,
            "Google のログインが切れました。もう一度ログインしてください",
        )),
        Some(error) => Err(CommandError::new(
            ErrorKind::Google,
            format!(
                "Google がトークンを発行しませんでした: {} {}",
                error.error,
                error.error_description.unwrap_or_default()
            ),
        )),
        None => Err(CommandError::new(
            ErrorKind::Google,
            format!("Google がトークンを発行しませんでした ({status})"),
        )),
    }
}

pub async fn exchange_code(
    http: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<TokenResponse> {
    token_request(
        http,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", redirect_uri),
        ],
    )
    .await
}

pub async fn refresh(
    http: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<TokenResponse> {
    token_request(
        http,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
        ],
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_matches_rfc_7636_example() {
        let pkce = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into());
        assert_eq!(
            pkce.challenge,
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn new_verifier_is_within_rfc_length() {
        let pkce = Pkce::new();
        assert!((43..=128).contains(&pkce.verifier.len()));
        assert_ne!(pkce.verifier, Pkce::new().verifier);
    }

    #[test]
    fn authorization_url_carries_every_parameter() {
        let url = Url::parse(&authorization_url(
            "id.apps",
            "http://127.0.0.1:5000",
            "chal",
            "st",
        ))
        .unwrap();
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params["client_id"], "id.apps");
        assert_eq!(params["redirect_uri"], "http://127.0.0.1:5000");
        assert_eq!(params["response_type"], "code");
        assert_eq!(params["scope"], SCOPE);
        assert_eq!(params["code_challenge"], "chal");
        assert_eq!(params["code_challenge_method"], "S256");
        assert_eq!(params["state"], "st");
        assert_eq!(params["access_type"], "offline");
    }

    #[test]
    fn redirect_with_code_and_matching_state() {
        assert_eq!(
            parse_redirect("GET /?state=st&code=4%2Fabc&scope=x HTTP/1.1", "st"),
            Redirect::Code("4/abc".into())
        );
    }

    #[test]
    fn redirect_with_error() {
        assert_eq!(
            parse_redirect("GET /?error=access_denied&state=st HTTP/1.1", "st"),
            Redirect::Denied("access_denied".into())
        );
    }

    #[test]
    fn redirect_with_wrong_state_is_ignored() {
        assert_eq!(
            parse_redirect("GET /?state=other&code=c HTTP/1.1", "st"),
            Redirect::Ignore
        );
        assert_eq!(
            parse_redirect("GET /?code=c HTTP/1.1", "st"),
            Redirect::Ignore
        );
    }

    #[test]
    fn other_requests_are_ignored() {
        assert_eq!(
            parse_redirect("GET /favicon.ico HTTP/1.1", "st"),
            Redirect::Ignore
        );
        assert_eq!(
            parse_redirect("POST /?state=st&code=c HTTP/1.1", "st"),
            Redirect::Ignore
        );
        assert_eq!(parse_redirect("", "st"), Redirect::Ignore);
    }
}
