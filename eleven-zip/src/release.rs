//! 7-Zip's releases: which is the newest, which file of it eleven-zip installs,
//! and whether a download is that file.
//!
//! 7-Zip's author publishes each release on GitHub (ip7z/7zip). eleven-zip asks
//! GitHub's API for the latest one and installs its 64-bit MSI package.
//! Nothing here touches the network or Win32, so it is tested on every
//! platform; win/net.rs does the requests.

use std::fmt;
use std::str::FromStr;

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// GitHub's API for the newest release that is neither a draft nor a
/// prerelease.
pub const LATEST_URL: &str = "https://api.github.com/repos/ip7z/7zip/releases/latest";
/// Where every file of a release is downloaded from. An answer pointing
/// anywhere else is refused.
const DOWNLOAD_PREFIX: &str = "https://github.com/ip7z/7zip/releases/download/";

/// How long after a check the next one is due, in seconds.
pub const CHECK_INTERVAL: u64 = 24 * 60 * 60;

/// A 7-Zip version, as its releases are tagged: `26.04`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02}", self.major, self.minor)
    }
}

impl FromStr for Version {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || format!("7-Zip のバージョンとして読めません: {s}");
        let (major, minor) = s.trim().split_once('.').ok_or_else(bad)?;
        let number = |part: &str| {
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad());
            }
            part.parse().map_err(|_| bad())
        };
        Ok(Version {
            major: number(major)?,
            minor: number(minor)?,
        })
    }
}

impl Version {
    /// The 64-bit MSI package of this version, as the release names it:
    /// `7z2604-x64.msi`.
    pub fn msi_name(self) -> String {
        format!("7z{}{:02}-x64.msi", self.major, self.minor)
    }
}

/// The newest release, as far as eleven-zip needs it.
#[derive(Debug, PartialEq)]
pub struct Release {
    pub version: Version,
    pub msi_url: String,
    /// The package's SHA-256 in lowercase hex, when GitHub lists one.
    pub sha256: Option<String>,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}

/// Reads GitHub's answer for the latest release.
pub fn parse_latest(json: &str) -> Result<Release, String> {
    let release: ApiRelease =
        serde_json::from_str(json).map_err(|e| format!("GitHub の応答を読めません: {e}"))?;
    let version: Version = release.tag_name.parse()?;
    let name = version.msi_name();
    let asset = release
        .assets
        .into_iter()
        .find(|a| a.name == name)
        .ok_or_else(|| format!("7-Zip {version} に {name} がありません"))?;
    if !asset.browser_download_url.starts_with(DOWNLOAD_PREFIX) {
        return Err(format!(
            "7-Zip の配布元ではない URL です: {}",
            asset.browser_download_url
        ));
    }
    let sha256 = asset
        .digest
        .and_then(|d| d.strip_prefix("sha256:").map(str::to_ascii_lowercase));
    Ok(Release {
        version,
        msi_url: asset.browser_download_url,
        sha256,
    })
}

/// Whether `bytes` are what `release` says its package is. A release that
/// lists no digest cannot be checked, and passes.
pub fn verify(release: &Release, bytes: &[u8]) -> Result<(), String> {
    let Some(expected) = &release.sha256 else {
        return Ok(());
    };
    let actual: String = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if &actual == expected {
        Ok(())
    } else {
        Err(format!(
            "ダウンロードした {} が壊れています",
            release.version.msi_name()
        ))
    }
}

/// Whether it is time to look for a new version again, `last` and `now`
/// being seconds since the Unix epoch. A clock set back counts as due.
pub fn is_due(last: Option<u64>, now: u64) -> bool {
    match last {
        None => true,
        Some(last) => now < last || now - last >= CHECK_INTERVAL,
    }
}

/// An `https://` URL's host and the path after it, which WinHTTP takes
/// separately.
pub fn split_https(url: &str) -> Option<(&str, &str)> {
    let rest = url.strip_prefix("https://")?;
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    (!host.is_empty()).then_some((host, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LATEST: &str = r#"{
        "tag_name": "26.04",
        "name": "7-Zip 26.04",
        "prerelease": false,
        "assets": [
            {
                "name": "7z2604-arm64.exe",
                "browser_download_url": "https://github.com/ip7z/7zip/releases/download/26.04/7z2604-arm64.exe",
                "digest": "sha256:d4117b95495b0e925334ae7b5bddf2853513274603de15dee6acf03999fbc09c"
            },
            {
                "name": "7z2604-x64.msi",
                "browser_download_url": "https://github.com/ip7z/7zip/releases/download/26.04/7z2604-x64.msi",
                "digest": "sha256:0B01334A418654293513449F61D6BFC99E5196CA371E3C3DC961EDA57CD535C6",
                "size": 2009088
            }
        ]
    }"#;

    #[test]
    fn versions() {
        let v: Version = "26.04".parse().unwrap();
        assert_eq!(
            v,
            Version {
                major: 26,
                minor: 4
            }
        );
        assert_eq!(v.to_string(), "26.04");
        assert_eq!(v.msi_name(), "7z2604-x64.msi");
        assert_eq!(
            "9.20".parse::<Version>().unwrap().msi_name(),
            "7z920-x64.msi"
        );
        assert!("25.01".parse::<Version>().unwrap() < v);
        assert!("26.10".parse::<Version>().unwrap() > v);
        for bad in ["", "26", "26.", ".04", "v26.04", "26.04.1", "26.0a"] {
            assert!(bad.parse::<Version>().is_err(), "{bad}");
        }
    }

    #[test]
    fn the_latest_release_and_its_msi() {
        let r = parse_latest(LATEST).unwrap();
        assert_eq!(r.version.to_string(), "26.04");
        assert_eq!(
            r.msi_url,
            "https://github.com/ip7z/7zip/releases/download/26.04/7z2604-x64.msi"
        );
        assert_eq!(
            r.sha256.as_deref(),
            Some("0b01334a418654293513449f61d6bfc99e5196ca371e3c3dc961eda57cd535c6")
        );
    }

    #[test]
    fn a_release_without_digests_still_reads() {
        let json = LATEST.replace("\"digest\"", "\"other\"");
        assert_eq!(parse_latest(&json).unwrap().sha256, None);
    }

    #[test]
    fn a_release_without_the_msi_is_an_error() {
        let json = LATEST.replace("7z2604-x64.msi", "7z2604-x64.zip");
        assert!(parse_latest(&json).unwrap_err().contains("7z2604-x64.msi"));
    }

    #[test]
    fn a_download_from_anywhere_else_is_refused() {
        let json = LATEST.replace(
            "https://github.com/ip7z/7zip/releases/download/26.04/7z2604-x64.msi",
            "https://example.com/7z2604-x64.msi",
        );
        assert!(parse_latest(&json).is_err());
    }

    #[test]
    fn something_that_is_not_a_release_is_an_error() {
        assert!(parse_latest(r#"{"message": "API rate limit exceeded"}"#).is_err());
        assert!(parse_latest("<html>").is_err());
    }

    #[test]
    fn digests() {
        let release = |sha256: Option<&str>| Release {
            version: "26.04".parse().unwrap(),
            msi_url: String::new(),
            sha256: sha256.map(str::to_string),
        };
        // SHA-256 of "abc".
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(verify(&release(Some(abc)), b"abc").is_ok());
        assert!(verify(&release(Some(abc)), b"abd").is_err());
        assert!(verify(&release(None), b"anything").is_ok());
    }

    #[test]
    fn checks_are_due_once_a_day() {
        assert!(is_due(None, 1_000));
        assert!(!is_due(Some(1_000), 1_000));
        assert!(!is_due(Some(1_000), 1_000 + CHECK_INTERVAL - 1));
        assert!(is_due(Some(1_000), 1_000 + CHECK_INTERVAL));
        assert!(is_due(Some(1_000), 999));
    }

    #[test]
    fn https_urls() {
        assert_eq!(
            split_https("https://api.github.com/repos/x?y=1"),
            Some(("api.github.com", "/repos/x?y=1"))
        );
        assert_eq!(
            split_https("https://example.com"),
            Some(("example.com", "/"))
        );
        assert_eq!(split_https("http://example.com/"), None);
        assert_eq!(split_https("https:///x"), None);
    }
}
