//! Which links in a message may leave Mail Crew. A link never navigates the
//! view the message is shown in; one that passes this is handed to the
//! default browser or mail client instead.

/// Whether a link in a message may be handed to the OS: http, https and
/// mailto, nothing else.
pub fn is_openable(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https" | "mailto"))
}

#[cfg(test)]
mod tests {
    use super::is_openable;

    #[test]
    fn only_web_and_mail_links_open() {
        assert!(is_openable("https://example.com/x?y=1"));
        assert!(is_openable("http://example.com"));
        assert!(is_openable("mailto:a@example.com"));
        assert!(!is_openable("javascript:alert(1)"));
        assert!(!is_openable("file:///etc/passwd"));
        assert!(!is_openable("data:text/html,<script>alert(1)</script>"));
        assert!(!is_openable("smb://host/share"));
        assert!(!is_openable("not a url"));
    }
}
