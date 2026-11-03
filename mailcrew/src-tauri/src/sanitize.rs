//! Cleaning a message's HTML before it reaches the webview.
//!
//! This is the first of two layers. Here everything that could run or submit
//! anything is removed: scripts, event handlers, forms, frames, embeds,
//! `javascript:` links, `<meta>` and `<base>`. The page then shows what is left
//! inside a sandboxed iframe that may not run scripts at all and whose
//! Content-Security-Policy refuses every load but inline styles and, until the
//! user asks for them, `data:` images (`src/message-frame.ts`).
//!
//! Styles stay. Mail is laid out with inline styles and `<style>` blocks, and
//! CSS cannot run code; what it could fetch, the iframe's policy blocks.

use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use ammonia::{Builder, UrlRelative};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sanitized {
    pub html: String,
    /// Whether the message would load images from the network, which the
    /// iframe blocks until the user lets it.
    pub remote_images: bool,
}

/// Presentational tags and attributes that mail still uses for layout, on top
/// of ammonia's defaults.
const EXTRA_TAGS: &[&str] = &["style", "font", "center", "big"];
const EXTRA_ATTRIBUTES: &[&str] = &[
    "style",
    "class",
    "align",
    "valign",
    "bgcolor",
    "background",
    "width",
    "height",
    "border",
    "cellpadding",
    "cellspacing",
    "color",
    "face",
    "size",
    "dir",
    "role",
    "title",
];

fn is_remote(url: &str) -> bool {
    let url = url.trim_start().to_ascii_lowercase();
    url.starts_with("http:") || url.starts_with("https:") || url.starts_with("//")
}

/// Whether a stylesheet or a `style` attribute pulls something in by URL.
fn css_loads_remote(css: &str) -> bool {
    let css = css.to_ascii_lowercase();
    css.match_indices("url(").any(|(at, _)| {
        let rest = css[at + 4..].trim_start().trim_start_matches(['"', '\'']);
        is_remote(rest)
    }) || css.contains("@import")
}

pub fn sanitize_html(input: &str) -> Sanitized {
    let remote = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&remote);
    let mut builder = Builder::default();
    builder
        .add_tags(EXTRA_TAGS)
        .rm_clean_content_tags(&["style"])
        // A <title> in the message's head would otherwise turn up as body text.
        .add_clean_content_tags(&["title"])
        .add_generic_attributes(EXTRA_ATTRIBUTES)
        .add_url_schemes(&["cid", "data"])
        .url_relative(UrlRelative::Deny)
        .link_rel(Some("noopener noreferrer"))
        .attribute_filter(move |element, attribute, value| {
            let loads = match (element, attribute) {
                ("img", "src") | (_, "background") => is_remote(value),
                (_, "style") => css_loads_remote(value),
                _ => false,
            };
            if loads {
                seen.store(true, Ordering::Relaxed);
            }
            Some(Cow::Borrowed(value))
        });
    let html = builder.clean(input).to_string();
    // `<style>` blocks are text to ammonia, so their URLs are looked for here.
    let remote_images = remote.load(Ordering::Relaxed) || css_loads_remote(&styles_of(&html));
    Sanitized {
        html,
        remote_images,
    }
}

/// The contents of every `<style>` element in already-sanitised HTML.
fn styles_of(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut css = String::new();
    let mut from = 0;
    while let Some(start) = lower[from..].find("<style") {
        let start = from + start;
        let Some(open_end) = lower[start..].find('>') else {
            break;
        };
        let body = start + open_end + 1;
        let end = lower[body..]
            .find("</style")
            .map_or(lower.len(), |end| body + end);
        css.push_str(&html[body..end]);
        from = end;
    }
    css
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The tags left in a sanitised message.
    fn tags_in(html: &str) -> HashSet<String> {
        let mut tags = HashSet::new();
        for piece in html.split('<').skip(1) {
            let name: String = piece
                .trim_start_matches('/')
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if !name.is_empty() {
                tags.insert(name.to_ascii_lowercase());
            }
        }
        tags
    }

    #[test]
    fn removes_everything_that_runs() {
        let out = sanitize_html(concat!(
            "<script>alert(1)</script>",
            "<p onclick=\"alert(1)\" onmouseover=x>hi</p>",
            "<a href=\"javascript:alert(1)\">js</a>",
            "<iframe src=\"https://example.com\"></iframe>",
            "<object data=x></object><embed src=x>",
            "<form action=\"https://example.com\"><input name=q><button>go</button></form>",
            "<meta http-equiv=\"refresh\" content=\"0;url=https://example.com\">",
            "<base href=\"https://example.com/\">",
            "<svg><script>alert(1)</script></svg>",
            "<link rel=stylesheet href=\"https://example.com/a.css\">",
        ))
        .html;
        let lower = out.to_ascii_lowercase();
        for banned in [
            "script", "iframe", "object", "embed", "form", "input", "button", "meta", "base",
            "svg", "link",
        ] {
            assert!(!tags_in(&out).contains(banned), "{banned} survived: {out}");
        }
        assert!(
            !lower.contains("onclick") && !lower.contains("onmouseover"),
            "{out}"
        );
        assert!(!lower.contains("javascript:"), "{out}");
        assert!(!lower.contains("alert"), "{out}");
    }

    #[test]
    fn keeps_mail_layout() {
        let out = sanitize_html(concat!(
            "<html><head><title>Newsletter</title><style>.x { color: red }</style></head>",
            "<body><table width=\"600\" bgcolor=\"#fff\" cellpadding=\"4\"><tr>",
            "<td align=\"center\" style=\"font-size: 14px\"><font color=\"red\">hi</font></td>",
            "</tr></table></body></html>",
        ))
        .html;
        assert!(out.contains("<style>.x { color: red }</style>"), "{out}");
        assert!(out.contains("width=\"600\""), "{out}");
        assert!(out.contains("bgcolor=\"#fff\""), "{out}");
        assert!(out.contains("style=\"font-size: 14px\""), "{out}");
        assert!(out.contains("<font color=\"red\">"), "{out}");
        assert!(!out.contains("Newsletter"), "the title became text: {out}");
    }

    #[test]
    fn links_stay_but_cannot_reach_back() {
        let out = sanitize_html(
            "<a href=\"https://example.com/x\">x</a><a href=\"mailto:a@example.com\">m</a>",
        )
        .html;
        assert!(out.contains("href=\"https://example.com/x\""), "{out}");
        assert!(out.contains("rel=\"noopener noreferrer\""), "{out}");
        assert!(out.contains("href=\"mailto:a@example.com\""), "{out}");
    }

    #[test]
    fn relative_urls_are_dropped() {
        let out = sanitize_html("<img src=\"/track.gif\"><a href=\"page.html\">x</a>").html;
        assert!(
            !out.contains("track.gif") && !out.contains("page.html"),
            "{out}"
        );
    }

    #[test]
    fn notices_remote_images() {
        assert!(sanitize_html("<img src=\"https://example.com/a.png\">").remote_images);
        // Protocol-relative is relative too, so it goes rather than loads.
        let protocol_relative = sanitize_html("<img src=\"//example.com/a.png\">");
        assert!(
            !protocol_relative.html.contains("example.com"),
            "{}",
            protocol_relative.html
        );
        assert!(
            sanitize_html(
                "<table><tr><td background=\"http://example.com/bg.png\"></td></tr></table>"
            )
            .remote_images
        );
        assert!(
            sanitize_html("<div style=\"background: url('https://example.com/a.png')\"></div>")
                .remote_images
        );
        assert!(
            sanitize_html("<style>.a { background-image: url(https://example.com/a.png) }</style>")
                .remote_images
        );
        assert!(
            !sanitize_html("<img src=\"data:image/png;base64,AAAA\"><p>text</p>").remote_images
        );
        assert!(!sanitize_html("<img src=\"cid:logo\">").remote_images);
    }
}
