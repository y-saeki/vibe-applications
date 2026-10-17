//! Where an archive is extracted to, and what 7-Zip's answers mean.
//!
//! Nothing here touches Win32 or runs 7-Zip, so it is tested on every
//! platform. win/extract.rs runs 7-Zip and hands its output here.

use std::path::{Path, PathBuf};

/// Where to extract an archive, and what Explorer opens afterwards.
#[derive(Debug, PartialEq)]
pub struct Plan {
    /// Passed to 7-Zip as the output folder (`-o`).
    pub output: PathBuf,
    /// The folder or file the archive turns into: the output folder itself,
    /// or the single item an archive holds, extracted next to it.
    pub result: PathBuf,
}

/// Works out where `archive` goes.
///
/// `entries` are the paths inside it, as 7-Zip lists them, or `None` when it
/// could not list them (an archive whose names are encrypted, say). With
/// `unwrap_single_item`, an archive whose entries all sit under one name is
/// extracted next to itself, as long as nothing there has that name already.
/// Anything else goes into a new folder named after the archive.
pub fn plan(
    archive: &Path,
    entries: Option<&[String]>,
    unwrap_single_item: bool,
    exists: impl Fn(&Path) -> bool,
) -> Plan {
    let parent = archive.parent().unwrap_or(Path::new("")).to_path_buf();
    if unwrap_single_item && let Some(name) = entries.and_then(single_top_level) {
        let result = parent.join(name);
        if !exists(&result) {
            return Plan {
                output: parent,
                result,
            };
        }
    }
    let file_name = archive
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let folder = unique(&parent, &folder_name(&file_name), &exists);
    Plan {
        output: folder.clone(),
        result: folder,
    }
}

/// The one name every entry sits under, if there is one: a single file, or
/// a single folder with everything else inside it.
fn single_top_level(entries: &[String]) -> Option<&str> {
    let mut names = entries
        .iter()
        .filter_map(|e| e.split(['\\', '/']).find(|part| !part.is_empty()));
    let first = names.next()?;
    if first == "." || first == ".." || first.ends_with(':') {
        return None;
    }
    names.all(|n| n == first).then_some(first)
}

/// `name` in `parent`, or `name (2)`, `name (3)` and so on: the first that
/// is not taken.
fn unique(parent: &Path, name: &str, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let first = parent.join(name);
    if !exists(&first) {
        return first;
    }
    (2..)
        .map(|n| parent.join(format!("{name} ({n})")))
        .find(|p| !exists(p))
        .expect("some number is free")
}

/// The folder an archive is extracted into when it gets one of its own: its
/// name without the extensions that say it is an archive. `photos.tar.gz`,
/// `photos.7z.001` and `photos.part1.rar` all become `photos`.
pub fn folder_name(file_name: &str) -> String {
    let mut name = file_name;
    // A volume of a split archive: `.001`.
    if let Some((stem, ext)) = name.rsplit_once('.')
        && ext.len() == 3
        && ext.bytes().all(|b| b.is_ascii_digit())
    {
        name = stem;
    }
    // A volume of a RAR archive: `.part1.rar`.
    if let Some(stem) = strip_suffix_ignore_case(name, ".rar")
        && let Some((before, part)) = stem.rsplit_once('.')
        && part.len() > 4
        && part[..4].eq_ignore_ascii_case("part")
        && part[4..].bytes().all(|b| b.is_ascii_digit())
    {
        name = before;
    } else if let Some((stem, _ext)) = name.rsplit_once('.')
        && !stem.is_empty()
    {
        name = stem;
    }
    if let Some(stem) = strip_suffix_ignore_case(name, ".tar")
        && !stem.is_empty()
    {
        name = stem;
    }
    // Windows drops trailing dots and spaces from a folder's name.
    let name = name.trim_end_matches(['.', ' ']);
    if name.is_empty() {
        format!("{file_name} の展開")
    } else {
        name.to_string()
    }
}

fn strip_suffix_ignore_case<'a>(s: &'a str, suffix: &str) -> Option<&'a str> {
    let at = s.len().checked_sub(suffix.len())?;
    (s.is_char_boundary(at) && s[at..].eq_ignore_ascii_case(suffix)).then(|| &s[..at])
}

/// The paths in the output of `7z l -ba -slt`, which lists each entry as a
/// block of `Name = value` lines, the first of them `Path`.
pub fn parse_listing(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| line.strip_prefix("Path = "))
        .map(str::to_string)
        .collect()
}

/// How a 7-Zip run ended, from its exit code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Done,
    /// Extracted, with warnings 7-Zip has already shown.
    Warnings,
    /// Stopped from 7-Zip's own window.
    Cancelled,
    /// Failed; 7-Zip has already said why.
    Failed,
}

impl Outcome {
    /// From 7-Zip's documented exit codes. A process that did not exit with
    /// a code (killed, say) failed.
    pub fn from_code(code: Option<i32>) -> Outcome {
        match code {
            Some(0) => Outcome::Done,
            Some(1) => Outcome::Warnings,
            Some(255) => Outcome::Cancelled,
            _ => Outcome::Failed,
        }
    }

    /// Whether there is something to show in Explorer.
    pub fn extracted(self) -> bool {
        matches!(self, Outcome::Done | Outcome::Warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| p.to_string()).collect()
    }

    fn nothing(_: &Path) -> bool {
        false
    }

    #[test]
    fn folder_names() {
        for (file, folder) in [
            ("photos.zip", "photos"),
            ("photos.ZIP", "photos"),
            ("photos.7z", "photos"),
            ("photos.tar.gz", "photos"),
            ("photos.TAR.XZ", "photos"),
            ("photos.tgz", "photos"),
            ("photos.tar", "photos"),
            ("photos.7z.001", "photos"),
            ("photos.zip.002", "photos"),
            ("photos.part1.rar", "photos"),
            ("photos.Part01.RAR", "photos"),
            ("photos.rar", "photos"),
            ("v1.2.0.zip", "v1.2.0"),
            ("report.party.rar", "report.party"),
            ("noext", "noext"),
            ("trailing. .zip", "trailing"),
            ("写真.zip", "写真"),
            (".zip", ".zip"),
            ("..zip", "..zip の展開"),
        ] {
            assert_eq!(folder_name(file), folder, "{file}");
        }
    }

    #[test]
    fn a_single_folder_is_extracted_next_to_the_archive() {
        let list = entries(&["photos", r"photos\a.jpg", r"photos\sub\b.jpg"]);
        let p = plan(Path::new("/dl/photos-2024.zip"), Some(&list), true, nothing);
        assert_eq!(p.output, Path::new("/dl"));
        assert_eq!(p.result, Path::new("/dl/photos"));
    }

    #[test]
    fn a_single_file_is_extracted_next_to_the_archive() {
        let list = entries(&["notes.txt"]);
        let p = plan(Path::new("/dl/notes.txt.gz"), Some(&list), true, nothing);
        assert_eq!(p.output, Path::new("/dl"));
        assert_eq!(p.result, Path::new("/dl/notes.txt"));
    }

    #[test]
    fn entries_listed_without_their_folder_still_count_as_one() {
        // Some archives list only files, with no entry for the folder.
        let list = entries(&["photos/a.jpg", "photos/b.jpg"]);
        let p = plan(Path::new("/dl/x.zip"), Some(&list), true, nothing);
        assert_eq!(p.result, Path::new("/dl/photos"));
    }

    #[test]
    fn several_items_get_a_folder_named_after_the_archive() {
        let list = entries(&["a.txt", "b.txt"]);
        let p = plan(Path::new("/dl/notes.zip"), Some(&list), true, nothing);
        assert_eq!(p.output, Path::new("/dl/notes"));
        assert_eq!(p.result, p.output);
    }

    #[test]
    fn a_single_item_gets_a_folder_when_unwrapping_is_off() {
        let list = entries(&["photos", r"photos\a.jpg"]);
        let p = plan(Path::new("/dl/photos.zip"), Some(&list), false, nothing);
        assert_eq!(p.output, Path::new("/dl/photos"));
    }

    #[test]
    fn a_single_item_whose_name_is_taken_gets_a_folder() {
        let list = entries(&["photos", r"photos\a.jpg"]);
        let taken = |p: &Path| p == Path::new("/dl/photos");
        let p = plan(Path::new("/dl/photos.zip"), Some(&list), true, taken);
        // The archive's own name is taken too, by that same folder.
        assert_eq!(p.output, Path::new("/dl/photos (2)"));
    }

    #[test]
    fn a_taken_folder_name_is_numbered() {
        let taken = |p: &Path| p == Path::new("/dl/x") || p == Path::new("/dl/x (2)");
        let p = plan(Path::new("/dl/x.zip"), None, true, taken);
        assert_eq!(p.output, Path::new("/dl/x (3)"));
    }

    #[test]
    fn an_archive_that_could_not_be_listed_gets_a_folder() {
        let p = plan(Path::new("/dl/secret.7z"), None, true, nothing);
        assert_eq!(p.output, Path::new("/dl/secret"));
    }

    #[test]
    fn an_empty_archive_gets_a_folder() {
        let p = plan(Path::new("/dl/empty.zip"), Some(&[]), true, nothing);
        assert_eq!(p.output, Path::new("/dl/empty"));
    }

    #[test]
    fn names_that_are_not_an_item_are_not_unwrapped() {
        for name in ["..", ".", "C:"] {
            let list = entries(&[&format!(r"{name}\a.txt")]);
            let p = plan(Path::new("/dl/x.zip"), Some(&list), true, nothing);
            assert_eq!(p.output, Path::new("/dl/x"), "{name}");
        }
    }

    #[test]
    fn listing() {
        let output = "\
Path = photos
Folder = +
Size = 0

Path = photos\\a.jpg
Folder = -
Size = 12
";
        assert_eq!(parse_listing(output), entries(&["photos", r"photos\a.jpg"]));
        assert!(parse_listing("").is_empty());
    }

    #[test]
    fn exit_codes() {
        assert_eq!(Outcome::from_code(Some(0)), Outcome::Done);
        assert_eq!(Outcome::from_code(Some(1)), Outcome::Warnings);
        assert_eq!(Outcome::from_code(Some(2)), Outcome::Failed);
        assert_eq!(Outcome::from_code(Some(8)), Outcome::Failed);
        assert_eq!(Outcome::from_code(Some(255)), Outcome::Cancelled);
        assert_eq!(Outcome::from_code(None), Outcome::Failed);
        assert!(Outcome::Done.extracted() && Outcome::Warnings.extracted());
        assert!(!Outcome::Cancelled.extracted() && !Outcome::Failed.extracted());
    }
}
