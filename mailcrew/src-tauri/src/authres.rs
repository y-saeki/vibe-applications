//! What Gmail found when it checked a message's sender: the SPF, DKIM and
//! DMARC results it records in an `Authentication-Results` header (RFC 8601).
//!
//! Anyone can put such a header in a message before sending it, so only the
//! one Gmail added is read: the topmost, since every receiver prepends its
//! own, and only when it names Gmail's authentication service.

use serde::Serialize;

/// The authentication service identifier Gmail writes at the start of its
/// `Authentication-Results` header.
pub const GMAIL_AUTHSERV_ID: &str = "mx.google.com";

/// One method's verdict, such as `spf=pass`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    /// The result keyword in lower case: `pass`, `fail`, `softfail`,
    /// `neutral`, `none`, `temperror`, `permerror` or `policy`.
    pub result: String,
    /// The domain the result is about (`smtp.mailfrom`, `header.d`,
    /// `header.from`); empty when the header does not say.
    pub domain: String,
}

/// The results Gmail recorded; a method it did not report is `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Authentication {
    pub spf: Option<Check>,
    pub dkim: Option<Check>,
    pub dmarc: Option<Check>,
}

impl Authentication {
    /// From the values of a message's `Authentication-Results` headers, in
    /// the order they appear in the message.
    pub fn from_headers<'a>(values: impl IntoIterator<Item = &'a str>) -> Self {
        let Some(first) = values.into_iter().next() else {
            return Self::default();
        };
        let (authserv_id, results) = parse(first);
        if !authserv_id.eq_ignore_ascii_case(GMAIL_AUTHSERV_ID) {
            return Self::default();
        }
        let pick = |method: &str| -> Option<Check> {
            let mut found = results.iter().filter(|result| result.method == method);
            let first = found.clone().next()?;
            // A message can carry several DKIM signatures; one that holds is
            // what counts.
            let chosen = found
                .find(|result| result.result == "pass")
                .unwrap_or(first);
            Some(Check {
                result: chosen.result.clone(),
                domain: chosen.domain(),
            })
        };
        Self {
            spf: pick("spf"),
            dkim: pick("dkim"),
            dmarc: pick("dmarc"),
        }
    }

    /// Whether DMARC passed for the author's domain.
    pub fn dmarc_passed(&self) -> bool {
        self.dmarc
            .as_ref()
            .is_some_and(|check| check.result == "pass")
    }
}

#[derive(Debug, PartialEq, Eq)]
struct MethodResult {
    method: String,
    result: String,
    /// `ptype.property` → value, both in lower case for the key.
    properties: Vec<(String, String)>,
}

impl MethodResult {
    fn property(&self, name: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn domain(&self) -> String {
        let value = match self.method.as_str() {
            "spf" => self
                .property("smtp.mailfrom")
                .or_else(|| self.property("smtp.helo")),
            "dkim" => self
                .property("header.d")
                .or_else(|| self.property("header.i")),
            "dmarc" => self.property("header.from"),
            _ => None,
        };
        let value = value.unwrap_or_default();
        // `smtp.mailfrom` and `header.i` may carry a local part.
        value
            .rsplit_once('@')
            .map_or(value, |(_, domain)| domain)
            .to_ascii_lowercase()
    }
}

/// Removes `(comments)`, which may nest, outside quoted strings.
fn strip_comments(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut depth = 0usize;
    let mut quoted = false;
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' if quoted || depth > 0 => {
                let next = chars.next();
                if depth == 0 {
                    out.push(ch);
                    out.extend(next);
                }
            }
            '"' if depth == 0 => {
                quoted = !quoted;
                out.push(ch);
            }
            '(' if !quoted => depth += 1,
            ')' if !quoted && depth > 0 => {
                depth -= 1;
                // A comment stands where whitespace could.
                if depth == 0 {
                    out.push(' ');
                }
            }
            _ if depth > 0 => {}
            _ => out.push(ch),
        }
    }
    out
}

/// Splits on `separator` outside quoted strings.
fn split_unquoted(value: &str, separator: char) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut quoted = false;
    for ch in value.chars() {
        if ch == '"' {
            quoted = !quoted;
        }
        if ch == separator && !quoted {
            parts.push(String::new());
        } else {
            parts.last_mut().expect("never empty").push(ch);
        }
    }
    parts
}

/// The tokens of one result: whitespace-separated, with any whitespace
/// around `=` and `.` taken out so that `spf = pass` is one token.
fn tokens(resinfo: &str) -> Vec<String> {
    let mut joined = String::with_capacity(resinfo.len());
    for word in resinfo.split_whitespace() {
        let glue = joined.ends_with(['=', '.', '/']) || word.starts_with(['=', '.', '/']);
        if !joined.is_empty() && !glue {
            joined.push(' ');
        }
        joined.push_str(word);
    }
    joined.split(' ').map(str::to_string).collect()
}

fn unquote(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(value)
        .to_string()
}

/// The authentication service identifier and each method's result.
fn parse(value: &str) -> (String, Vec<MethodResult>) {
    let value = strip_comments(value);
    let mut parts = split_unquoted(&value, ';').into_iter();
    let authserv_id = parts
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string();
    let mut results = Vec::new();
    for part in parts {
        let mut tokens = tokens(&part).into_iter().filter(|token| !token.is_empty());
        let Some(spec) = tokens.next() else { continue };
        let Some((method, result)) = spec.split_once('=') else {
            // `; none`: no checks were made.
            continue;
        };
        // `method/version`: the version does not change what is read.
        let method = method.split('/').next().unwrap_or_default();
        let properties = tokens
            .filter_map(|token| {
                let (key, value) = token.split_once('=')?;
                key.contains('.')
                    .then(|| (key.to_ascii_lowercase(), unquote(value)))
            })
            .collect();
        results.push(MethodResult {
            method: method.to_ascii_lowercase(),
            result: result.to_ascii_lowercase(),
            properties,
        });
    }
    (authserv_id, results)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GMAIL: &str = "mx.google.com;\r\n       dkim=pass header.i=@news.example.com header.s=s1 header.b=AbCd;\r\n       spf=pass (google.com: domain of bounce@mail.example.com designates 192.0.2.1 as permitted sender) smtp.mailfrom=bounce@mail.example.com;\r\n       dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=example.com";

    #[test]
    fn reads_gmails_results() {
        let auth = Authentication::from_headers([GMAIL]);
        assert_eq!(
            auth,
            Authentication {
                spf: Some(Check {
                    result: "pass".into(),
                    domain: "mail.example.com".into()
                }),
                dkim: Some(Check {
                    result: "pass".into(),
                    domain: "news.example.com".into()
                }),
                dmarc: Some(Check {
                    result: "pass".into(),
                    domain: "example.com".into()
                }),
            }
        );
        assert!(auth.dmarc_passed());
    }

    #[test]
    fn only_the_topmost_header_counts() {
        // A forged header further down is ignored even when it claims Gmail.
        let forged = "mx.google.com; dmarc=pass header.from=bank.example";
        let auth = Authentication::from_headers([
            "mx.google.com; spf=softfail smtp.mailfrom=x.example; dmarc=fail header.from=bank.example",
            forged,
        ]);
        assert_eq!(auth.dmarc.unwrap().result, "fail");
        assert_eq!(auth.spf.unwrap().result, "softfail");
        assert!(auth.dkim.is_none());
    }

    #[test]
    fn a_header_from_another_service_is_ignored() {
        let auth =
            Authentication::from_headers(["evil.example; dmarc=pass header.from=bank.example"]);
        assert_eq!(auth, Authentication::default());
        assert!(!auth.dmarc_passed());
        assert_eq!(Authentication::from_headers([]), Authentication::default());
    }

    #[test]
    fn a_passing_dkim_signature_wins() {
        let auth = Authentication::from_headers([
            "mx.google.com; dkim=fail header.d=old.example; dkim = pass (good) header.d=Example.COM; spf=none",
        ]);
        assert_eq!(
            auth.dkim,
            Some(Check {
                result: "pass".into(),
                domain: "example.com".into()
            })
        );
        assert_eq!(
            auth.spf,
            Some(Check {
                result: "none".into(),
                domain: String::new()
            })
        );
    }

    #[test]
    fn comments_and_quotes_do_not_confuse_it() {
        let (id, results) = parse(
            "mx.google.com 1 (a; b (nested; c)); spf=pass (x=y; z) smtp.mailfrom=\"a;b\"@example.com",
        );
        assert_eq!(id, "mx.google.com");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].domain(), "example.com");
        assert_eq!(parse("mx.google.com; none").1, Vec::new());
    }
}
