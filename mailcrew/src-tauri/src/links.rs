//! Keeping the webview on mailcrew's own page.
//!
//! A link clicked inside a message's frame is normally caught by the page
//! (`src/message-frame.ts`) and handed to `open_link`. WebKit, however, does
//! not run the page's listeners inside a frame sandboxed without
//! `allow-scripts`, so on macOS the click goes ahead as a navigation of the
//! frame. WKWebView asks about every frame's navigation, so it ends up here:
//! anything that is not the app's own page is opened in the default browser
//! instead, or refused.
//!
//! WebKit only asks once the navigation has passed the page's `frame-src`, and
//! drops one the policy refuses without asking. `tauri.macos.conf.json` lets
//! `http:`, `https:` and `mailto:` through so that a link reaches this guard.
//!
//! WebView2 only asks about the top-level page, which is why the page still
//! catches clicks itself.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;
use tauri_plugin_opener::OpenerExt;
use url::Url;

use crate::commands::is_openable;

#[derive(Debug, PartialEq, Eq)]
pub enum Navigation {
    /// The app's own page, or a message's `srcdoc` document.
    Allow,
    /// A web or mail link: opened outside, never in the webview.
    OpenOutside,
    Refuse,
}

pub fn decide(url: &Url) -> Navigation {
    let own = match url.scheme() {
        "tauri" | "about" => true,
        "http" | "https" => {
            matches!(url.host_str(), Some("tauri.localhost"))
                // `devUrl` in tauri.conf.json, only while developing.
                || (cfg!(debug_assertions)
                    && url.host_str() == Some("127.0.0.1")
                    && url.port() == Some(1420))
        }
        _ => false,
    };
    if own {
        Navigation::Allow
    } else if is_openable(url.as_str()) {
        Navigation::OpenOutside
    } else {
        Navigation::Refuse
    }
}

pub fn guard<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("link-guard")
        .on_navigation(|webview, url| match decide(url) {
            Navigation::Allow => true,
            Navigation::OpenOutside => {
                if let Err(err) = webview.opener().open_url(url.as_str(), None::<&str>) {
                    eprintln!("mailcrew: could not open {url}: {err}");
                }
                false
            }
            Navigation::Refuse => false,
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decide_str(url: &str) -> Navigation {
        decide(&Url::parse(url).unwrap())
    }

    #[test]
    fn the_app_and_message_documents_stay() {
        assert_eq!(
            decide_str("tauri://localhost/index.html"),
            Navigation::Allow
        );
        assert_eq!(decide_str("http://tauri.localhost/"), Navigation::Allow);
        assert_eq!(decide_str("about:srcdoc"), Navigation::Allow);
        assert_eq!(decide_str("about:blank"), Navigation::Allow);
    }

    #[test]
    fn web_and_mail_links_open_outside() {
        assert_eq!(
            decide_str("https://example.com/news?id=1"),
            Navigation::OpenOutside
        );
        assert_eq!(decide_str("http://example.com/"), Navigation::OpenOutside);
        assert_eq!(decide_str("mailto:a@example.com"), Navigation::OpenOutside);
        // A look-alike host is somebody else's site.
        assert_eq!(
            decide_str("https://tauri.localhost.example.com/"),
            Navigation::OpenOutside
        );
    }

    #[test]
    fn everything_else_is_refused() {
        assert_eq!(decide_str("data:text/html,<p>x</p>"), Navigation::Refuse);
        assert_eq!(decide_str("file:///etc/passwd"), Navigation::Refuse);
        assert_eq!(decide_str("javascript:alert(1)"), Navigation::Refuse);
    }
}
