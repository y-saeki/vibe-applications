//! What eleven-zip was started to do, from its command line.
//!
//! Nothing here touches Win32, so it is tested on every platform.

use std::ffi::OsString;
use std::path::PathBuf;

/// The argument Explorer passes before an archive, from the verbs shell.rs
/// registers.
pub const EXTRACT: &str = "--extract";
/// Run by the installer after copying the files, and by the uninstaller
/// before removing them.
pub const REGISTER: &str = "--register";
pub const UNREGISTER: &str = "--unregister";

#[derive(Debug, PartialEq)]
pub enum Command {
    /// No arguments: the settings window, as the Start menu opens it.
    Settings,
    Extract(Vec<PathBuf>),
    Register,
    Unregister,
}

/// Reads the arguments after the program name. Files with no switch in
/// front, as when they are dropped onto eleven-zip.exe, are extracted.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Command {
    let mut args = args.into_iter().peekable();
    let Some(first) = args.peek() else {
        return Command::Settings;
    };
    match first.to_str() {
        Some(REGISTER) => Command::Register,
        Some(UNREGISTER) => Command::Unregister,
        Some(EXTRACT) => Command::Extract(args.skip(1).map(PathBuf::from).collect()),
        _ => Command::Extract(args.map(PathBuf::from).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Command {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn nothing_opens_the_settings() {
        assert_eq!(parse_strs(&[]), Command::Settings);
    }

    #[test]
    fn switches() {
        assert_eq!(parse_strs(&["--register"]), Command::Register);
        assert_eq!(parse_strs(&["--unregister"]), Command::Unregister);
        assert_eq!(
            parse_strs(&["--extract", r"C:\a b\x.zip"]),
            Command::Extract(vec![PathBuf::from(r"C:\a b\x.zip")])
        );
    }

    #[test]
    fn bare_files_are_extracted() {
        assert_eq!(
            parse_strs(&["x.zip", "y.7z"]),
            Command::Extract(vec![PathBuf::from("x.zip"), PathBuf::from("y.7z")])
        );
    }

    #[test]
    fn extract_with_no_file_extracts_nothing() {
        assert_eq!(parse_strs(&["--extract"]), Command::Extract(vec![]));
    }
}
