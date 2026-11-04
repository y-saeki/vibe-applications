//! BIMI: the logo a sender's domain publishes for its mail, shown only when a
//! mark certificate from a recognised authority vouches for it.
//!
//! The steps follow draft-brand-indicators-for-message-identification-15
//! (section 7) and draft-fetch-validation-vmc-wchuang-12:
//!
//! 1. Gmail's DMARC result for the message must be `pass`, and the domain's
//!    DMARC policy must be enforced (`quarantine` at 100 % or `reject`).
//! 2. The BIMI record is looked up in DNS: `default._bimi.<author domain>`,
//!    then the organisational domain. A local-part selector (`lps=`) is
//!    followed. The `BIMI-Selector` header is not read: it counts only when a
//!    DKIM signature covers it, which Gmail does not report.
//! 3. The certificate in `a=` is fetched over HTTPS and checked: its chain to a
//!    BIMI root, its validity, the BIMI extended key usage on it and on its
//!    issuer, the CRL it names, and that it names the record's domain.
//! 4. The logo is the SVG embedded in the certificate. The `l=` file is never
//!    fetched, so a record without a certificate shows no logo.
//!
//! Certificate Transparency proofs are not checked.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use hickory_resolver::TokioResolver;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, TrustAnchor, UnixTime};
use serde::Serialize;
use tokio::sync::{Mutex, OnceCell};
use x509_parser::prelude::*;

/// id-kp-BrandIndicatorforMessageIdentification (1.3.6.1.5.5.7.3.31).
const BIMI_EKU: &[u8] = &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x1f];
const BIMI_EKU_OID: &str = "1.3.6.1.5.5.7.3.31";
/// The logotype extension (RFC 3709).
const LOGOTYPE_OID: &str = "1.3.6.1.5.5.7.1.12";
/// `subject:markType` in the Mark Certificate Requirements.
const MARK_TYPE_OID: &str = "1.3.6.1.4.1.53087.1.13";

/// The roots of the mark verifying authorities the AuthIndicators Working
/// Group lists (https://bimigroup.org/vmc-issuers/).
const ROOTS: &[&[u8]] = &[
    include_bytes!("../bimi-roots/digicert-verified-mark-root-ca.pem"),
    include_bytes!("../bimi-roots/entrust-verified-mark-root-vmcr1.pem"),
    include_bytes!("../bimi-roots/globalsign-verified-mark-root-r42.pem"),
    include_bytes!("../bimi-roots/sslcom-vmc-ecc-root-ca-2024.pem"),
    include_bytes!("../bimi-roots/sslcom-vmc-rsa-root-ca-2024.pem"),
];

/// A certificate and its chain is a few kilobytes plus the logo, which the
/// BIMI SVG profile keeps under 32 KB.
const MAX_EVIDENCE: usize = 256 * 1024;
const MAX_CRL: usize = 4 * 1024 * 1024;
const MAX_LOGO: usize = 64 * 1024;
/// How long a sender's result is kept before it is looked up again.
const CACHE_FOR: Duration = Duration::from_secs(60 * 60);

/// The outcome of the BIMI checks, as `bimi=` in RFC 8601 terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BimiResult {
    /// The domain publishes a BIMI record and its DMARC policy qualifies.
    Pass,
    /// No BIMI record.
    None,
    /// The domain published a record that declines BIMI.
    Declined,
    /// The record is malformed, or there is more than one.
    Fail,
    /// Not looked up: DMARC did not pass, or the domain does not enforce it.
    Skipped,
    /// DNS did not answer.
    Temperror,
}

/// What the record's mark certificate came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MarkResult {
    /// The record names no certificate, or BIMI did not pass.
    None,
    /// A Verified Mark Certificate: a registered or government mark.
    Vmc,
    /// A Common Mark Certificate: a mark verified by prior use and the like.
    Cmc,
    /// The certificate did not hold up.
    Fail,
    /// The certificate or its CRL could not be fetched.
    Temperror,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Brand {
    pub bimi: BimiResult,
    /// The domain whose BIMI record was used; empty when none was.
    pub domain: String,
    pub mark: MarkResult,
    /// The certificate's logo as a `data:image/svg+xml` URL.
    pub logo: Option<String>,
}

impl Brand {
    fn without(bimi: BimiResult) -> Self {
        Self {
            bimi,
            domain: String::new(),
            mark: MarkResult::None,
            logo: None,
        }
    }
}

/// A DNS lookup that did not come back with an answer, as against one that
/// came back with no records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DnsFailed;

/// TXT records, each one's strings joined.
pub trait Dns {
    async fn txt(&self, name: &str) -> Result<Vec<String>, DnsFailed>;
}

/// A fetch that did not bring back the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchFailed;

pub trait Fetch {
    async fn get(&self, url: &str, limit: usize) -> Result<Vec<u8>, FetchFailed>;
}

// --- Records ---------------------------------------------------------------

/// `key=value` pairs separated by `;`, keys in lower case. `None` when the
/// first tag is not `v=<version>`.
fn tags(record: &str, version: &str) -> Option<Vec<(String, String)>> {
    let tags: Vec<(String, String)> = record
        .split(';')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(|tag| {
            let (key, value) = tag.split_once('=').unwrap_or((tag, ""));
            (key.trim().to_ascii_lowercase(), value.trim().to_string())
        })
        .collect();
    let (key, value) = tags.first()?;
    (key == "v" && value == version).then_some(tags)
}

fn tag<'a>(tags: &'a [(String, String)], key: &str) -> Option<&'a str> {
    tags.iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

/// The records at `name` that start with `v=<version>`.
async fn records(
    dns: &impl Dns,
    name: &str,
    version: &str,
) -> Result<Vec<Vec<(String, String)>>, DnsFailed> {
    Ok(dns
        .txt(name)
        .await?
        .iter()
        .filter_map(|record| tags(record, version))
        .collect())
}

/// The registrable domain under the public suffix list, which DMARC and BIMI
/// call the organisational domain.
fn organizational(domain: &str) -> String {
    psl::domain_str(domain).unwrap_or(domain).to_string()
}

/// Whether the domain's DMARC policy is strict enough for BIMI.
async fn dmarc_qualifies(dns: &impl Dns, author: &str, org: &str) -> Result<bool, DnsFailed> {
    let mut found = records(dns, &format!("_dmarc.{author}"), "DMARC1").await?;
    let mut inherited = false;
    if found.is_empty() && author != org {
        found = records(dns, &format!("_dmarc.{org}"), "DMARC1").await?;
        inherited = true;
    }
    let [record] = found.as_slice() else {
        return Ok(false);
    };
    let p = tag(record, "p").unwrap_or_default().to_ascii_lowercase();
    let sp = tag(record, "sp").map(str::to_ascii_lowercase);
    // A subdomain is held to `sp` when the organisational domain sets one.
    let policy = match (&sp, inherited) {
        (Some(sp), true) => sp.clone(),
        _ => p.clone(),
    };
    if p == "none" || sp.as_deref() == Some("none") {
        return Ok(false);
    }
    match policy.as_str() {
        "reject" => Ok(true),
        "quarantine" => Ok(tag(record, "pct").is_none_or(|pct| pct.trim() == "100")),
        _ => Ok(false),
    }
}

/// The local part as a selector (draft section 4.5), or `None` when it has
/// characters a selector cannot.
fn local_part_selector(local: &str) -> Option<String> {
    let local = local.split('+').next().unwrap_or_default();
    let mut out = String::new();
    for ch in local.chars() {
        let ch = if ch == '_' || ch == '.' { '-' } else { ch };
        if ch == '-' && out.ends_with('-') {
            continue;
        }
        out.push(ch.to_ascii_lowercase());
    }
    let out = out.trim_matches('-').to_string();
    let valid = !out.is_empty()
        && out.len() <= 63
        && out
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-');
    valid.then_some(out)
}

/// A BIMI record together with where it was found.
struct Assertion {
    tags: Vec<(String, String)>,
    selector: String,
    domain: String,
}

/// Looks up the record at `domain`, following `lps=` when it is there.
/// `Ok(None)` when there is no single record.
async fn assertion_at(
    dns: &impl Dns,
    domain: &str,
    local: &str,
) -> Result<Option<Result<Assertion, ()>>, DnsFailed> {
    let found = records(dns, &format!("default._bimi.{domain}"), "BIMI1").await?;
    let record = match found.as_slice() {
        [] => return Ok(None),
        [record] => record.clone(),
        _ => return Ok(Some(Err(()))),
    };
    if let Some(prefixes) = tag(&record, "lps") {
        let prefixes: Vec<String> = prefixes
            .split(',')
            .map(|prefix| prefix.trim().to_ascii_lowercase())
            .filter(|prefix| !prefix.is_empty())
            .collect();
        if let Some(selector) = local_part_selector(local) {
            let matches = prefixes.is_empty()
                || prefixes
                    .iter()
                    .any(|prefix| selector.starts_with(prefix.as_str()));
            if matches && selector != "default" {
                let by_local = records(dns, &format!("{selector}._bimi.{domain}"), "BIMI1").await?;
                if let [found] = by_local.as_slice() {
                    return Ok(Some(Ok(Assertion {
                        tags: found.clone(),
                        selector,
                        domain: domain.to_string(),
                    })));
                }
            }
        }
    }
    Ok(Some(Ok(Assertion {
        tags: record,
        selector: "default".to_string(),
        domain: domain.to_string(),
    })))
}

fn https_url(value: &str) -> Option<String> {
    let url = url::Url::parse(value.trim()).ok()?;
    (url.scheme() == "https" && url.host_str().is_some()).then(|| url.to_string())
}

// --- Mark certificates -----------------------------------------------------

/// A certificate that passed every check that does not depend on the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    pub kind: MarkResult,
    /// SAN dNSNames in lower case.
    pub names: Vec<String>,
    pub logo: String,
}

/// Why a certificate was turned down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkError {
    /// Fetching the CRL failed; worth trying again later.
    Unreachable,
    Invalid(String),
}

fn invalid(reason: impl std::fmt::Display) -> MarkError {
    MarkError::Invalid(reason.to_string())
}

fn trust_anchors(roots: &[CertificateDer<'static>]) -> Vec<TrustAnchor<'static>> {
    roots
        .iter()
        .filter_map(|root| webpki::anchor_from_trusted_cert(root).ok())
        .map(|anchor| anchor.to_owned())
        .collect()
}

pub fn bimi_roots() -> Vec<CertificateDer<'static>> {
    ROOTS
        .iter()
        .flat_map(|pem| CertificateDer::pem_slice_iter(pem).filter_map(Result::ok))
        .collect()
}

fn has_bimi_eku(cert: &X509Certificate<'_>) -> bool {
    matches!(cert.extended_key_usage(), Ok(Some(eku))
        if eku.value.other.iter().any(|oid| oid.to_id_string() == BIMI_EKU_OID))
}

/// The first DER element in `der`: its tag, its content and what follows.
fn tlv(der: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let tag = *der.first()?;
    let first = *der.get(1)?;
    let (len, header) = if first < 0x80 {
        (first as usize, 2)
    } else {
        let count = (first & 0x7f) as usize;
        if count == 0 || count > 3 {
            return None;
        }
        let bytes = der.get(2..2 + count)?;
        (
            bytes
                .iter()
                .fold(0usize, |len, byte| (len << 8) | *byte as usize),
            2 + count,
        )
    };
    let content = der.get(header..header + len)?;
    Some((tag, content, &der[header + len..]))
}

/// Walks DER and collects the IA5Strings in it.
fn ia5_strings(mut der: &[u8], out: &mut Vec<String>) {
    while let Some((tag, content, rest)) = tlv(der) {
        if tag == 0x16 {
            if let Ok(text) = std::str::from_utf8(content) {
                out.push(text.to_string());
            }
        } else if tag & 0x20 != 0 {
            ia5_strings(content, out);
        }
        der = rest;
    }
}

/// The subject logo of a logotype extension: `subjectLogo [2]` holds the
/// image's `data:` URL among its details.
fn subject_logo(extension: &[u8]) -> Option<Vec<u8>> {
    // LogotypeExtn ::= SEQUENCE { ..., subjectLogo [2] EXPLICIT LogotypeInfo, ... }
    let (0x30, mut fields, _) = tlv(extension)? else {
        return None;
    };
    let mut outer = Vec::new();
    while let Some((tag, content, rest)) = tlv(fields) {
        if tag == 0xa2 {
            ia5_strings(content, &mut outer);
        }
        fields = rest;
    }
    let url = outer
        .iter()
        .find(|url| url.to_ascii_lowercase().starts_with("data:image/svg+xml"))?;
    let (meta, data) = url.split_once(',')?;
    let bytes = if meta.to_ascii_lowercase().ends_with(";base64") {
        base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .ok()?
    } else {
        data.as_bytes().to_vec()
    };
    // The requirements have the SVG compressed; take it either way.
    let svg = if bytes.starts_with(&[0x1f, 0x8b]) {
        use std::io::Read as _;
        let mut svg = Vec::new();
        flate2::read::GzDecoder::new(bytes.as_slice())
            .take(MAX_LOGO as u64 + 1)
            .read_to_end(&mut svg)
            .ok()?;
        svg
    } else {
        bytes
    };
    (svg.len() <= MAX_LOGO && String::from_utf8_lossy(&svg).contains("<svg")).then_some(svg)
}

/// The http(s) URLs in the certificate's CRL distribution points.
fn crl_urls(cert: &X509Certificate<'_>) -> Vec<String> {
    let mut urls = Vec::new();
    for extension in cert.extensions() {
        if let ParsedExtension::CRLDistributionPoints(points) = extension.parsed_extension() {
            for point in points.iter() {
                if let Some(DistributionPointName::FullName(names)) = &point.distribution_point {
                    for name in names {
                        if let GeneralName::URI(uri) = name {
                            if uri.starts_with("http://") || uri.starts_with("https://") {
                                urls.push(uri.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    urls
}

/// Checks a fetched evidence document: the PEM chain, its path to `roots`,
/// the end-entity certificate's profile, and its revocation through the CRL
/// it names, which `fetch` brings.
pub async fn verify_mark(
    pem: &[u8],
    roots: &[CertificateDer<'static>],
    now: UnixTime,
    fetch: &impl Fetch,
) -> Result<Mark, MarkError> {
    let chain: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(pem)
        .collect::<Result<_, _>>()
        .map_err(|_| invalid("証明書を読み取れませんでした"))?;
    let parsed: Vec<X509Certificate<'_>> = chain
        .iter()
        .map(|der| parse_x509_certificate(der).map(|(_, cert)| cert))
        .collect::<Result<_, _>>()
        .map_err(|_| invalid("証明書を読み取れませんでした"))?;
    // The mark certificate comes first; the rest are its issuers.
    let (Some(end), Some(cert)) = (chain.first(), parsed.first()) else {
        return Err(invalid("証明書がありません"));
    };
    if !has_bimi_eku(cert) {
        return Err(invalid("BIMI 用の証明書ではありません"));
    }
    let issuer = parsed[1..]
        .iter()
        .find(|other| other.subject().as_raw() == cert.issuer().as_raw())
        .ok_or_else(|| invalid("発行元の証明書がありません"))?;
    if !has_bimi_eku(issuer) {
        return Err(invalid("発行元が BIMI 用の認証局ではありません"));
    }

    let urls = crl_urls(cert);
    if urls.is_empty() {
        return Err(invalid("失効リストの場所がありません"));
    }
    let mut crl = None;
    for url in &urls {
        if let Ok(der) = fetch.get(url, MAX_CRL).await {
            crl = Some(der);
            break;
        }
    }
    let crl = crl.ok_or(MarkError::Unreachable)?;
    let crl = webpki::OwnedCertRevocationList::from_der(&crl)
        .map_err(|err| invalid(format!("失効リストを読み取れませんでした: {err:?}")))?;
    let crl = webpki::CertRevocationList::from(crl);
    let crls = [&crl];
    let revocation = webpki::RevocationOptionsBuilder::new(&crls)
        .map_err(|_| invalid("失効リストがありません"))?
        .with_depth(webpki::RevocationCheckDepth::EndEntity)
        .with_status_policy(webpki::UnknownStatusPolicy::Deny)
        .build();

    let anchors = trust_anchors(roots);
    let end_entity =
        webpki::EndEntityCert::try_from(end).map_err(|err| invalid(format!("{err:?}")))?;
    end_entity
        .verify_for_usage(
            webpki::ALL_VERIFICATION_ALGS,
            &anchors,
            &chain[1..],
            now,
            // Upper intermediates may carry no EKU at all; the mark
            // certificate and its issuer were checked for it above.
            webpki::KeyUsage::required_if_present(BIMI_EKU),
            Some(revocation),
            None,
        )
        .map_err(|err| match err {
            webpki::Error::CertRevoked => invalid("証明書は失効しています"),
            webpki::Error::CertExpired { .. } => invalid("証明書の期限が切れています"),
            webpki::Error::UnknownIssuer => invalid("認められた認証局の証明書ではありません"),
            other => invalid(format!("証明書を検証できませんでした: {other:?}")),
        })?;

    let logo = cert
        .extensions()
        .iter()
        .find(|extension| extension.oid.to_id_string() == LOGOTYPE_OID)
        .and_then(|extension| subject_logo(extension.value))
        .ok_or_else(|| invalid("証明書にロゴがありません"))?;
    let kind = match cert
        .subject()
        .iter_attributes()
        .find(|attribute| attribute.attr_type().to_id_string() == MARK_TYPE_OID)
        .and_then(|attribute| attribute.as_str().ok())
    {
        Some("Registered Mark" | "Government Mark") => MarkResult::Vmc,
        Some(_) => MarkResult::Cmc,
        // Before the requirements added markType, every mark certificate was
        // a VMC; none of those is still within its validity.
        None => MarkResult::Vmc,
    };
    let names = match cert.subject_alternative_name() {
        Ok(Some(san)) => san
            .value
            .general_names
            .iter()
            .filter_map(|name| match name {
                GeneralName::DNSName(name) => Some(name.trim_end_matches('.').to_ascii_lowercase()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    Ok(Mark {
        kind,
        names,
        logo: format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(logo)
        ),
    })
}

/// Whether the certificate names the domain the record was found at, either
/// on its own or with the record's selector (VMC draft section 5.2).
fn names_match(names: &[String], assertion: &Assertion, author: &str) -> bool {
    let mut domains = vec![assertion.domain.clone()];
    if !domains.iter().any(|domain| domain == author) {
        domains.push(author.to_string());
    }
    names.iter().any(|name| {
        domains.iter().any(|domain| {
            name == domain || *name == format!("{}._bimi.{domain}", assertion.selector)
        })
    })
}

// --- The whole check ---------------------------------------------------------

/// The brand for mail from `author` (an address in lower case), given whether
/// Gmail found DMARC to pass. `mark` fetches and checks the certificate at a
/// URL.
pub async fn evaluate(
    dns: &impl Dns,
    author: &str,
    dmarc_passed: bool,
    mark: impl AsyncFnOnce(&str) -> Result<Mark, MarkError>,
) -> Brand {
    if !dmarc_passed {
        return Brand::without(BimiResult::Skipped);
    }
    let Some((local, domain)) = author.rsplit_once('@') else {
        return Brand::without(BimiResult::Skipped);
    };
    let domain = domain.trim_end_matches('.').to_ascii_lowercase();
    let org = organizational(&domain);
    match dmarc_qualifies(dns, &domain, &org).await {
        Ok(true) => {}
        Ok(false) => return Brand::without(BimiResult::Skipped),
        Err(DnsFailed) => return Brand::without(BimiResult::Temperror),
    }
    let mut found = match assertion_at(dns, &domain, local).await {
        Ok(found) => found,
        Err(DnsFailed) => return Brand::without(BimiResult::Temperror),
    };
    if found.is_none() && org != domain {
        found = match assertion_at(dns, &org, local).await {
            Ok(found) => found,
            Err(DnsFailed) => return Brand::without(BimiResult::Temperror),
        };
    }
    let assertion = match found {
        None => return Brand::without(BimiResult::None),
        Some(Err(())) => return Brand::without(BimiResult::Fail),
        Some(Ok(assertion)) => assertion,
    };
    let l = tag(&assertion.tags, "l").unwrap_or_default();
    let a = tag(&assertion.tags, "a").unwrap_or_default();
    let mut brand = Brand {
        bimi: BimiResult::Pass,
        domain: assertion.domain.clone(),
        mark: MarkResult::None,
        logo: None,
    };
    if l.is_empty() && a.is_empty() {
        brand.bimi = BimiResult::Declined;
        return brand;
    }
    if https_url(l).is_none() {
        brand.bimi = BimiResult::Fail;
        return brand;
    }
    if a.is_empty() {
        return brand;
    }
    let Some(evidence) = https_url(a) else {
        brand.mark = MarkResult::Fail;
        return brand;
    };
    match mark(&evidence).await {
        Ok(found) if names_match(&found.names, &assertion, &domain) => {
            brand.mark = found.kind;
            brand.logo = Some(found.logo);
        }
        Ok(_) | Err(MarkError::Invalid(_)) => brand.mark = MarkResult::Fail,
        Err(MarkError::Unreachable) => brand.mark = MarkResult::Temperror,
    }
    brand
}

/// Fetches and checks the certificate at `url`.
pub async fn fetch_mark<F: Fetch>(
    fetch: &F,
    url: &str,
    roots: &[CertificateDer<'static>],
    now: UnixTime,
) -> Result<Mark, MarkError> {
    let pem = fetch
        .get(url, MAX_EVIDENCE)
        .await
        .map_err(|_| MarkError::Unreachable)?;
    verify_mark(&pem, roots, now, fetch).await
}

// --- The live service ----------------------------------------------------------

pub struct SystemDns {
    resolver: OnceCell<Option<TokioResolver>>,
}

impl Dns for SystemDns {
    async fn txt(&self, name: &str) -> Result<Vec<String>, DnsFailed> {
        let resolver = self
            .resolver
            .get_or_init(|| async {
                TokioResolver::builder_tokio()
                    .ok()
                    .and_then(|builder| builder.build().ok())
            })
            .await
            .as_ref()
            .ok_or(DnsFailed)?;
        // Fully qualified, so that no search domain is tried.
        match resolver.txt_lookup(format!("{name}.")).await {
            Ok(lookup) => Ok(lookup
                .answers()
                .iter()
                .filter_map(|record| match &record.data {
                    hickory_resolver::proto::rr::RData::TXT(txt) => Some(
                        txt.txt_data
                            .iter()
                            .map(|part| String::from_utf8_lossy(part))
                            .collect::<String>(),
                    ),
                    _ => None,
                })
                .collect()),
            Err(err) if err.is_no_records_found() || err.is_nx_domain() => Ok(Vec::new()),
            Err(_) => Err(DnsFailed),
        }
    }
}

pub struct HttpFetch {
    http: reqwest::Client,
}

impl Fetch for HttpFetch {
    async fn get(&self, url: &str, limit: usize) -> Result<Vec<u8>, FetchFailed> {
        let mut response = self.http.get(url).send().await.map_err(|_| FetchFailed)?;
        if !response.status().is_success() {
            return Err(FetchFailed);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| FetchFailed)? {
            body.extend_from_slice(&chunk);
            if body.len() > limit {
                return Err(FetchFailed);
            }
        }
        Ok(body)
    }
}

/// One key's value and when it was worked out; locked while it is.
type Slot<T> = Arc<Mutex<Option<(Instant, T)>>>;

/// A value kept for `CACHE_FOR`, worked out once at a time per key.
struct Memo<T> {
    slots: Mutex<HashMap<String, Slot<T>>>,
}

impl<T: Clone> Memo<T> {
    fn new() -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
        }
    }

    /// The kept value for `key`, or the one `make` works out. `keep` says
    /// whether a new value may be kept.
    async fn get(&self, key: &str, make: impl AsyncFnOnce() -> T, keep: impl Fn(&T) -> bool) -> T {
        let slot = {
            let mut slots = self.slots.lock().await;
            slots.retain(|_, slot| {
                slot.try_lock().map_or(true, |value| {
                    value
                        .as_ref()
                        .is_some_and(|(at, _)| at.elapsed() < CACHE_FOR)
                })
            });
            slots.entry(key.to_string()).or_default().clone()
        };
        let mut value = slot.lock().await;
        if let Some((at, kept)) = value.as_ref() {
            if at.elapsed() < CACHE_FOR {
                return kept.clone();
            }
        }
        let made = make().await;
        *value = keep(&made).then(|| (Instant::now(), made.clone()));
        made
    }
}

/// The BIMI checks as the app runs them: the system's DNS, HTTPS, the
/// bundled roots, and results kept for an hour.
pub struct Brands {
    dns: SystemDns,
    fetch: HttpFetch,
    roots: Vec<CertificateDer<'static>>,
    by_sender: Memo<Brand>,
    marks: Memo<Result<Mark, MarkError>>,
}

impl Brands {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            // Evidence documents are HTTPS only; a redirect may not leave it.
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() > 5 {
                    attempt.error("too many redirects")
                } else if attempt.previous()[0].scheme() == "https"
                    && attempt.url().scheme() != "https"
                {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .expect("the HTTP client builds with the default TLS backend");
        Self {
            dns: SystemDns {
                resolver: OnceCell::new(),
            },
            fetch: HttpFetch { http },
            roots: bimi_roots(),
            by_sender: Memo::new(),
            marks: Memo::new(),
        }
    }

    pub async fn brand(&self, author: &str, dmarc_passed: bool) -> Brand {
        if !dmarc_passed {
            return Brand::without(BimiResult::Skipped);
        }
        let now = UnixTime::now();
        self.by_sender
            .get(
                author,
                async || {
                    evaluate(&self.dns, author, true, async |url: &str| {
                        self.marks
                            .get(
                                url,
                                async || fetch_mark(&self.fetch, url, &self.roots, now).await,
                                |result| !matches!(result, Err(MarkError::Unreachable)),
                            )
                            .await
                    })
                    .await
                },
                |brand| brand.bimi != BimiResult::Temperror && brand.mark != MarkResult::Temperror,
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{
        BasicConstraints, CertificateParams, CertificateRevocationListParams, CertifiedIssuer,
        CrlDistributionPoint, CustomExtension, DnType, ExtendedKeyUsagePurpose, IsCa, KeyIdMethod,
        KeyPair, RevocationReason, RevokedCertParams, SanType, SerialNumber,
    };

    const CRL_URL: &str = "http://crl.ca.example/vmc.crl";

    #[derive(Default)]
    struct FakeDns {
        records: HashMap<String, Vec<String>>,
        broken: bool,
    }

    impl FakeDns {
        fn with(mut self, name: &str, record: &str) -> Self {
            self.records
                .entry(name.to_string())
                .or_default()
                .push(record.to_string());
            self
        }
    }

    impl Dns for FakeDns {
        async fn txt(&self, name: &str) -> Result<Vec<String>, DnsFailed> {
            if self.broken {
                return Err(DnsFailed);
            }
            Ok(self.records.get(name).cloned().unwrap_or_default())
        }
    }

    #[derive(Default)]
    struct FakeFetch {
        files: HashMap<String, Vec<u8>>,
    }

    impl Fetch for FakeFetch {
        async fn get(&self, url: &str, limit: usize) -> Result<Vec<u8>, FetchFailed> {
            match self.files.get(url) {
                Some(body) if body.len() <= limit => Ok(body.clone()),
                _ => Err(FetchFailed),
            }
        }
    }

    const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" version="1.2" baseProfile="tiny-ps"><title>Shop</title></svg>"#;

    /// DER for a LogotypeExtn whose subject logo is `url`.
    fn logotype(url: &str) -> Vec<u8> {
        fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
            let mut out = vec![tag];
            let len = content.len();
            if len < 0x80 {
                out.push(len as u8);
            } else if len < 0x100 {
                out.extend([0x81, len as u8]);
            } else {
                out.extend([0x82, (len >> 8) as u8, len as u8]);
            }
            out.extend_from_slice(content);
            out
        }
        // sha-256 AlgorithmIdentifier and a hash value; nothing reads them.
        let alg = tlv(
            0x30,
            &tlv(
                0x06,
                &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01],
            ),
        );
        let hash = tlv(0x30, &[alg, tlv(0x04, &[0; 32])].concat());
        let details = tlv(
            0x30,
            &[
                tlv(0x16, b"image/svg+xml"),
                tlv(0x30, &hash),
                tlv(0x30, &tlv(0x16, url.as_bytes())),
            ]
            .concat(),
        );
        let image = tlv(0x30, &details);
        let data = tlv(0xa0, &tlv(0x30, &image));
        tlv(0x30, &tlv(0xa2, &data))
    }

    fn gzipped_logo() -> String {
        use std::io::Write as _;
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(SVG.as_bytes()).unwrap();
        let bytes = gz.finish().unwrap();
        format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }

    struct Pki {
        roots: Vec<CertificateDer<'static>>,
        intermediate: CertifiedIssuer<'static, KeyPair>,
    }

    fn ca(name: &str, bimi: bool) -> CertificateParams {
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.distinguished_name.push(DnType::CommonName, name);
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        if bimi {
            params.extended_key_usages = vec![ExtendedKeyUsagePurpose::Other(vec![
                1, 3, 6, 1, 5, 5, 7, 3, 31,
            ])];
        }
        params.key_identifier_method = KeyIdMethod::Sha256;
        params
    }

    fn make_pki() -> Pki {
        let root =
            CertifiedIssuer::self_signed(ca("Test BIMI Root", false), KeyPair::generate().unwrap())
                .unwrap();
        let intermediate = CertifiedIssuer::signed_by(
            ca("Test BIMI Intermediate", true),
            KeyPair::generate().unwrap(),
            &root,
        )
        .unwrap();
        Pki {
            roots: vec![root.der().clone()],
            intermediate,
        }
    }

    struct Leaf {
        names: Vec<&'static str>,
        mark_type: Option<&'static str>,
        eku: bool,
        logo: Option<String>,
        serial: u64,
        expired: bool,
    }

    impl Default for Leaf {
        fn default() -> Self {
            Self {
                names: vec!["shop.example"],
                mark_type: Some("Registered Mark"),
                eku: true,
                logo: Some(gzipped_logo()),
                serial: 7,
                expired: false,
            }
        }
    }

    fn leaf(pki: &Pki, leaf: Leaf) -> String {
        let mut params = CertificateParams::new(
            leaf.names
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        params.subject_alt_names = leaf
            .names
            .iter()
            .map(|name| SanType::DnsName((*name).try_into().unwrap()))
            .collect();
        params.distinguished_name.push(DnType::CommonName, "Shop");
        if let Some(mark_type) = leaf.mark_type {
            params.distinguished_name.push(
                DnType::CustomDnType(vec![1, 3, 6, 1, 4, 1, 53087, 1, 13]),
                mark_type,
            );
        }
        if leaf.eku {
            params.extended_key_usages = vec![ExtendedKeyUsagePurpose::Other(vec![
                1, 3, 6, 1, 5, 5, 7, 3, 31,
            ])];
        }
        if let Some(logo) = &leaf.logo {
            params.custom_extensions = vec![CustomExtension::from_oid_content(
                &[1, 3, 6, 1, 5, 5, 7, 1, 12],
                logotype(logo),
            )];
        }
        params.crl_distribution_points = vec![CrlDistributionPoint {
            uris: vec![CRL_URL.to_string()],
        }];
        params.serial_number = Some(SerialNumber::from(leaf.serial));
        if leaf.expired {
            params.not_before = rcgen::date_time_ymd(2020, 1, 1);
            params.not_after = rcgen::date_time_ymd(2021, 1, 1);
        }
        let cert = params
            .signed_by(&KeyPair::generate().unwrap(), &pki.intermediate)
            .unwrap();
        format!("{}{}", cert.pem(), pki.intermediate.pem())
    }

    fn crl(pki: &Pki, revoked: &[u64]) -> Vec<u8> {
        let now = rcgen::date_time_ymd(2026, 1, 1);
        CertificateRevocationListParams {
            this_update: now,
            next_update: rcgen::date_time_ymd(2099, 1, 1),
            crl_number: SerialNumber::from(1u64),
            issuing_distribution_point: None,
            revoked_certs: revoked
                .iter()
                .map(|serial| RevokedCertParams {
                    serial_number: SerialNumber::from(*serial),
                    revocation_time: now,
                    reason_code: Some(RevocationReason::KeyCompromise),
                    invalidity_date: None,
                })
                .collect(),
            key_identifier_method: KeyIdMethod::Sha256,
        }
        .signed_by(&pki.intermediate)
        .unwrap()
        .der()
        .to_vec()
    }

    fn fetch_with(pki: &Pki, pem: &str, revoked: &[u64]) -> FakeFetch {
        let mut fetch = FakeFetch::default();
        fetch.files.insert(
            "https://shop.example/vmc.pem".into(),
            pem.as_bytes().to_vec(),
        );
        fetch.files.insert(CRL_URL.into(), crl(pki, revoked));
        fetch
    }

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future)
    }

    async fn check(pki: &Pki, fetch: &FakeFetch) -> Result<Mark, MarkError> {
        fetch_mark(
            fetch,
            "https://shop.example/vmc.pem",
            &pki.roots,
            UnixTime::now(),
        )
        .await
    }

    #[test]
    fn a_valid_vmc_yields_its_logo() {
        let pki = make_pki();
        let fetch = fetch_with(&pki, &leaf(&pki, Leaf::default()), &[]);
        let mark = block_on(check(&pki, &fetch)).unwrap();
        assert_eq!(mark.kind, MarkResult::Vmc);
        assert_eq!(mark.names, vec!["shop.example".to_string()]);
        let encoded = mark
            .logo
            .strip_prefix("data:image/svg+xml;base64,")
            .unwrap();
        let svg = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        assert_eq!(svg, SVG.as_bytes());
    }

    #[test]
    fn a_prior_use_mark_is_a_cmc() {
        let pki = make_pki();
        let pem = leaf(
            &pki,
            Leaf {
                mark_type: Some("Prior Use Mark"),
                ..Leaf::default()
            },
        );
        let mark = block_on(check(&pki, &fetch_with(&pki, &pem, &[]))).unwrap();
        assert_eq!(mark.kind, MarkResult::Cmc);
    }

    #[test]
    fn certificates_that_do_not_hold_up_are_refused() {
        let pki = make_pki();
        let refused = |leaf_params: Leaf, revoked: &[u64]| {
            let fetch = fetch_with(&pki, &leaf(&pki, leaf_params), revoked);
            matches!(block_on(check(&pki, &fetch)), Err(MarkError::Invalid(_)))
        };
        assert!(refused(Leaf::default(), &[7]), "revoked");
        assert!(
            refused(
                Leaf {
                    expired: true,
                    ..Leaf::default()
                },
                &[]
            ),
            "expired"
        );
        assert!(
            refused(
                Leaf {
                    eku: false,
                    ..Leaf::default()
                },
                &[]
            ),
            "no BIMI EKU"
        );
        assert!(
            refused(
                Leaf {
                    logo: None,
                    ..Leaf::default()
                },
                &[]
            ),
            "no logo"
        );
        assert!(
            refused(
                Leaf {
                    logo: Some("data:image/png;base64,AAAA".into()),
                    ..Leaf::default()
                },
                &[]
            ),
            "not SVG"
        );
        // A chain to a root the app does not trust.
        let other = make_pki();
        let fetch = fetch_with(&other, &leaf(&other, Leaf::default()), &[]);
        assert!(matches!(
            block_on(check(&pki, &fetch)),
            Err(MarkError::Invalid(_))
        ));
    }

    #[test]
    fn an_unreachable_crl_is_tried_again_later() {
        let pki = make_pki();
        let mut fetch = fetch_with(&pki, &leaf(&pki, Leaf::default()), &[]);
        fetch.files.remove(CRL_URL);
        assert_eq!(block_on(check(&pki, &fetch)), Err(MarkError::Unreachable));
        fetch.files.clear();
        assert_eq!(block_on(check(&pki, &fetch)), Err(MarkError::Unreachable));
    }

    #[test]
    fn the_bundled_roots_load() {
        let roots = bimi_roots();
        assert_eq!(roots.len(), ROOTS.len());
        assert_eq!(trust_anchors(&roots).len(), ROOTS.len());
    }

    fn run(dns: &FakeDns, fetch: &FakeFetch, pki: &Pki, author: &str, dmarc: bool) -> Brand {
        block_on(evaluate(dns, author, dmarc, async |url: &str| {
            fetch_mark(fetch, url, &pki.roots, UnixTime::now()).await
        }))
    }

    const RECORD: &str = "v=BIMI1; l=https://shop.example/logo.svg; a=https://shop.example/vmc.pem";

    fn shop_dns() -> FakeDns {
        FakeDns::default()
            .with(
                "_dmarc.shop.example",
                "v=DMARC1; p=reject; rua=mailto:d@shop.example",
            )
            .with("default._bimi.shop.example", RECORD)
    }

    #[test]
    fn a_domain_with_a_vmc_shows_its_logo() {
        let pki = make_pki();
        let fetch = fetch_with(&pki, &leaf(&pki, Leaf::default()), &[]);
        let brand = run(&shop_dns(), &fetch, &pki, "news@shop.example", true);
        assert_eq!(brand.bimi, BimiResult::Pass);
        assert_eq!(brand.domain, "shop.example");
        assert_eq!(brand.mark, MarkResult::Vmc);
        assert!(brand
            .logo
            .unwrap()
            .starts_with("data:image/svg+xml;base64,"));
    }

    #[test]
    fn nothing_is_looked_up_unless_dmarc_passed() {
        let pki = make_pki();
        let dns = FakeDns {
            broken: true,
            ..FakeDns::default()
        };
        let brand = run(
            &dns,
            &FakeFetch::default(),
            &pki,
            "news@shop.example",
            false,
        );
        assert_eq!(brand, Brand::without(BimiResult::Skipped));
    }

    #[test]
    fn a_weak_dmarc_policy_skips_bimi() {
        let pki = make_pki();
        let fetch = fetch_with(&pki, &leaf(&pki, Leaf::default()), &[]);
        for policy in [
            "v=DMARC1; p=none",
            "v=DMARC1; p=quarantine; pct=50",
            "v=DMARC1; p=reject; sp=none",
            "v=DMARC2; p=reject",
        ] {
            let dns = FakeDns::default()
                .with("_dmarc.shop.example", policy)
                .with("default._bimi.shop.example", RECORD);
            let brand = run(&dns, &fetch, &pki, "news@shop.example", true);
            assert_eq!(brand.bimi, BimiResult::Skipped, "{policy}");
        }
        let dns = FakeDns::default()
            .with("_dmarc.shop.example", "v=DMARC1; p=quarantine; pct=100")
            .with("default._bimi.shop.example", RECORD);
        assert_eq!(
            run(&dns, &fetch, &pki, "news@shop.example", true).bimi,
            BimiResult::Pass
        );
    }

    #[test]
    fn a_subdomain_falls_back_to_the_organisational_domain() {
        let pki = make_pki();
        let fetch = fetch_with(&pki, &leaf(&pki, Leaf::default()), &[]);
        let dns = shop_dns();
        let brand = run(&dns, &fetch, &pki, "a@mail.shop.example", true);
        assert_eq!(brand.domain, "shop.example");
        assert_eq!(brand.mark, MarkResult::Vmc);
        // The organisational domain's sp= governs its subdomains.
        let dns = FakeDns::default()
            .with(
                "_dmarc.shop.example",
                "v=DMARC1; p=reject; sp=quarantine; pct=10",
            )
            .with("default._bimi.shop.example", RECORD);
        assert_eq!(
            run(&dns, &fetch, &pki, "a@mail.shop.example", true).bimi,
            BimiResult::Skipped
        );
    }

    #[test]
    fn records_say_none_declined_or_fail() {
        let pki = make_pki();
        let fetch = FakeFetch::default();
        let dmarc = || FakeDns::default().with("_dmarc.shop.example", "v=DMARC1; p=reject");
        assert_eq!(
            run(&dmarc(), &fetch, &pki, "a@shop.example", true).bimi,
            BimiResult::None
        );
        let declined = dmarc().with("default._bimi.shop.example", "v=BIMI1; l=; a=;");
        assert_eq!(
            run(&declined, &fetch, &pki, "a@shop.example", true).bimi,
            BimiResult::Declined
        );
        let two = dmarc()
            .with("default._bimi.shop.example", RECORD)
            .with("default._bimi.shop.example", RECORD);
        assert_eq!(
            run(&two, &fetch, &pki, "a@shop.example", true).bimi,
            BimiResult::Fail
        );
        let http = dmarc().with(
            "default._bimi.shop.example",
            "v=BIMI1; l=http://shop.example/l.svg",
        );
        assert_eq!(
            run(&http, &fetch, &pki, "a@shop.example", true).bimi,
            BimiResult::Fail
        );
        let broken = FakeDns {
            broken: true,
            ..FakeDns::default()
        };
        assert_eq!(
            run(&broken, &fetch, &pki, "a@shop.example", true).bimi,
            BimiResult::Temperror
        );
    }

    #[test]
    fn a_record_without_a_certificate_shows_no_logo() {
        let pki = make_pki();
        let dns = FakeDns::default()
            .with("_dmarc.shop.example", "v=DMARC1; p=reject")
            .with(
                "default._bimi.shop.example",
                "v=BIMI1; l=https://shop.example/logo.svg",
            );
        let brand = run(&dns, &FakeFetch::default(), &pki, "a@shop.example", true);
        assert_eq!(brand.bimi, BimiResult::Pass);
        assert_eq!(brand.mark, MarkResult::None);
        assert_eq!(brand.logo, None);
    }

    #[test]
    fn a_certificate_for_another_domain_is_refused() {
        let pki = make_pki();
        let pem = leaf(
            &pki,
            Leaf {
                names: vec!["other.example"],
                ..Leaf::default()
            },
        );
        let brand = run(
            &shop_dns(),
            &fetch_with(&pki, &pem, &[]),
            &pki,
            "a@shop.example",
            true,
        );
        assert_eq!(brand.mark, MarkResult::Fail);
        assert_eq!(brand.logo, None);
        // Naming the selector is as good as naming the domain.
        let pem = leaf(
            &pki,
            Leaf {
                names: vec!["default._bimi.shop.example"],
                ..Leaf::default()
            },
        );
        let brand = run(
            &shop_dns(),
            &fetch_with(&pki, &pem, &[]),
            &pki,
            "a@shop.example",
            true,
        );
        assert_eq!(brand.mark, MarkResult::Vmc);
    }

    #[test]
    fn an_unreachable_certificate_is_a_temporary_error() {
        let pki = make_pki();
        let brand = run(
            &shop_dns(),
            &FakeFetch::default(),
            &pki,
            "a@shop.example",
            true,
        );
        assert_eq!(brand.bimi, BimiResult::Pass);
        assert_eq!(brand.mark, MarkResult::Temperror);
    }

    #[test]
    fn a_local_part_selector_picks_its_own_record() {
        let pki = make_pki();
        let fetch = FakeFetch::default();
        let dns = FakeDns::default()
            .with("_dmarc.shop.example", "v=DMARC1; p=reject")
            .with("default._bimi.shop.example", "v=BIMI1; l=; a=; lps=brand-")
            .with(
                "brand-news._bimi.shop.example",
                "v=BIMI1; l=https://shop.example/news.svg",
            );
        let brand = run(&dns, &fetch, &pki, "Brand.News+tag@shop.example", true);
        assert_eq!(brand.bimi, BimiResult::Pass);
        // Any other local part keeps the declining default.
        assert_eq!(
            run(&dns, &fetch, &pki, "info@shop.example", true).bimi,
            BimiResult::Declined
        );
    }

    #[test]
    fn local_parts_normalise_into_selectors() {
        assert_eq!(
            local_part_selector("Brand_.News+x").as_deref(),
            Some("brand-news")
        );
        assert_eq!(local_part_selector("-a-").as_deref(), Some("a"));
        assert_eq!(local_part_selector("日本"), None);
        assert_eq!(local_part_selector(""), None);
    }
}
