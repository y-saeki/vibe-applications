//! Putting an attachment on disk: a file name the OS will take, a place that
//! does not overwrite anything, and which kinds of file Mail Crew opens.
//!
//! What a file does when it is opened is decided by its extension, and the
//! extension is the sender's choice. So only kinds that open in a viewer —
//! documents, images, audio and video — are ever opened from Mail Crew;
//! anything else (programs, scripts, installers, shortcuts, archives, web
//! pages) can only be saved, and the user decides what to do with it there.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The extensions an attachment may be opened with, in lower case.
const OPENABLE: &[&[&str]] = &[DOCUMENTS, IMAGES, MEDIA];
const DOCUMENTS: &[&str] = &[
    "pdf", "txt", "csv", "tsv", "md", "rtf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt",
    "ods", "odp", "pages", "numbers", "key",
];
const IMAGES: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "heic", "heif", "bmp", "tif", "tiff", "avif",
];
const MEDIA: &[&str] = &["mp3", "m4a", "aac", "wav", "flac", "mp4", "m4v", "mov"];

/// The longest file name written, in bytes; most file systems stop at 255.
const MAX_NAME_BYTES: usize = 200;

/// Names Windows keeps for devices, whatever the extension.
const RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// A name the attachment can be written under on macOS and Windows alike:
/// no directories, no characters either OS refuses, nothing hidden, no
/// trailing dots or spaces (which Windows drops, changing the extension).
pub fn safe_file_name(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = replaced
        .trim_matches(|c: char| c.is_whitespace() || c == '.')
        .to_string();
    let mut name = if trimmed.is_empty() {
        "attachment".to_string()
    } else {
        trimmed
    };
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if RESERVED.contains(&stem.trim()) {
        name.insert(0, '_');
    }
    shorten(&name)
}

/// Cuts the stem so that the whole name fits, keeping the extension.
fn shorten(name: &str) -> String {
    if name.len() <= MAX_NAME_BYTES {
        return name.to_string();
    }
    let (stem, extension) = split_extension(name);
    let extension = extension.map(|ext| format!(".{ext}")).unwrap_or_default();
    let room = MAX_NAME_BYTES.saturating_sub(extension.len());
    let mut cut = room.min(stem.len());
    while !stem.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}{extension}", &stem[..cut])
}

fn split_extension(name: &str) -> (&str, Option<&str>) {
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && !extension.is_empty() => {
            (stem, Some(extension))
        }
        _ => (name, None),
    }
}

/// Whether a file of this name may be handed to the OS to open.
pub fn is_openable(name: &str) -> bool {
    split_extension(&safe_file_name(name))
        .1
        .is_some_and(|extension| {
            let extension = extension.to_ascii_lowercase();
            OPENABLE
                .iter()
                .any(|kinds| kinds.contains(&extension.as_str()))
        })
}

/// Writes `bytes` to `dir` under `name`, or under `name (2)`, `name (3)` and
/// so on when that is taken. Nothing already there is overwritten.
pub fn write_new(dir: &Path, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let name = safe_file_name(name);
    let (stem, extension) = split_extension(&name);
    for n in 1..1000 {
        let candidate = match (n, extension) {
            (1, _) => name.clone(),
            (_, Some(extension)) => format!("{stem} ({n}).{extension}"),
            (_, None) => format!("{stem} ({n})"),
        };
        let path = dir.join(candidate);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(bytes)?;
                return Ok(path);
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "同じ名前のファイルが多すぎます",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_lose_what_the_os_would_refuse() {
        assert_eq!(safe_file_name("請求書.pdf"), "請求書.pdf");
        assert_eq!(safe_file_name("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(safe_file_name("a\\b:c*d?.txt"), "a_b_c_d_.txt");
        assert_eq!(safe_file_name(".hidden"), "hidden");
        assert_eq!(safe_file_name("report.exe. . "), "report.exe");
        assert_eq!(safe_file_name("CON.txt"), "_CON.txt");
        assert_eq!(safe_file_name(""), "attachment");
        assert_eq!(safe_file_name("tab\there.pdf"), "tab_here.pdf");
        let long = format!("{}.pdf", "あ".repeat(100));
        let short = safe_file_name(&long);
        assert!(
            short.len() <= MAX_NAME_BYTES && short.ends_with(".pdf"),
            "{short}"
        );
    }

    #[test]
    fn only_viewer_kinds_open() {
        for name in ["a.pdf", "B.JPG", "表.xlsx", "memo.txt", "clip.mov"] {
            assert!(is_openable(name), "{name}");
        }
        for name in [
            "setup.exe",
            "invoice.pdf.exe",
            "invoice.pdf.exe.",
            "run.bat",
            "x.js",
            "x.vbs",
            "x.ps1",
            "x.lnk",
            "x.app",
            "x.command",
            "x.dmg",
            "x.zip",
            "x.html",
            "x.svg",
            "x.docm",
            "x.ics",
            "noextension",
            ".pdf",
        ] {
            assert!(!is_openable(name), "{name}");
        }
    }

    #[test]
    fn never_overwrites() {
        let dir = std::env::temp_dir().join(format!("mailcrew-test-{}", rand::random::<u64>()));
        let first = write_new(&dir, "a.pdf", b"1").unwrap();
        let second = write_new(&dir, "a.pdf", b"2").unwrap();
        let bare = write_new(&dir, "README", b"3").unwrap();
        let bare_again = write_new(&dir, "README", b"4").unwrap();
        assert_eq!(first.file_name().unwrap(), "a.pdf");
        assert_eq!(second.file_name().unwrap(), "a (2).pdf");
        assert_eq!(bare_again.file_name().unwrap(), "README (2)");
        assert_eq!(std::fs::read(&first).unwrap(), b"1");
        assert_eq!(std::fs::read(&second).unwrap(), b"2");
        assert_eq!(std::fs::read(&bare).unwrap(), b"3");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
