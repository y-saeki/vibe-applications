//! Extracting the archives Explorer hands eleven-zip: lists each with 7z.exe to
//! decide where it goes (archive.rs), extracts it with 7zG.exe, whose own
//! window shows the progress and asks for a password or what to overwrite,
//! then shows the result in Explorer and puts the archive in the Recycle Bin.

use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_SILENT, FOF_WANTNUKEWARNING, SHFILEOPSTRUCTW,
    SHFileOperationW, SHOpenFolderAndSelectItems, SHParseDisplayName, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

use super::{config_path, error_box, sevenzip, store};
use crate::archive::{self, Outcome};
use crate::config::Config;
use crate::store::Installed;

/// A password for listing, so that 7z.exe fails at once on an archive whose
/// names are encrypted rather than waiting for one to be typed. Archives
/// whose names are not encrypted list the same with any password.
const LISTING_PASSWORD: &str = "-peleven-zip";

pub fn run(archives: &[PathBuf]) {
    if archives.is_empty() {
        return;
    }
    let config = Config::load(&config_path()).unwrap_or_else(|e| {
        error_box(&e.to_string());
        Config::default()
    });
    let store = store();
    let sevenzip = match sevenzip::ensure(&store) {
        Ok(installed) => installed,
        Err(e) => {
            error_box(&format!("7-Zip を用意できないため、展開できません。\n{e}"));
            return;
        }
    };
    for archive in archives {
        extract(&sevenzip, archive, &config);
    }
    sevenzip::update_if_due(&store, config.auto_update);
}

fn extract(sevenzip: &Installed, archive: &Path, config: &Config) {
    let entries = list(sevenzip, archive);
    let plan = archive::plan(
        archive,
        entries.as_deref(),
        config.unwrap_single_item,
        Path::exists,
    );
    let status = Command::new(sevenzip.gui())
        .arg("x")
        .arg(format!("-o{}", plan.output.display()))
        .arg("--")
        .arg(archive)
        .status();
    let outcome = match status {
        Ok(status) => Outcome::from_code(status.code()),
        Err(e) => {
            error_box(&format!("7-Zip を起動できません。\n{e}"));
            return;
        }
    };
    if outcome.extracted() && config.open_in_explorer && plan.result.exists() {
        show(&plan.result);
    }
    if outcome == Outcome::Done && config.trash_archive {
        trash(archive);
    }
}

/// Puts `archive`, with the other volumes of a split one, in the Recycle
/// Bin. Explorer asks first where there is no Recycle Bin to put it in, and
/// says itself when a file cannot be moved.
fn trash(archive: &Path) {
    let Ok(archive) = std::path::absolute(archive) else {
        return;
    };
    let siblings = archive
        .parent()
        .and_then(|dir| dir.read_dir().ok())
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok());
    // A list of paths, each ending in a null, and the list in another.
    let mut from: Vec<u16> = Vec::new();
    for path in archive::volumes(&archive, siblings) {
        from.extend(path.as_os_str().encode_wide());
        from.push(0);
    }
    from.push(0);
    let mut op = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(from.as_ptr()),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_WANTNUKEWARNING).0 as u16,
        ..Default::default()
    };
    unsafe { SHFileOperationW(&mut op) };
}

/// The paths inside `archive`, or `None` when 7z.exe cannot list them.
fn list(sevenzip: &Installed, archive: &Path) -> Option<Vec<String>> {
    let output = Command::new(sevenzip.cli())
        .args(["l", "-ba", "-slt", "-sccUTF-8", LISTING_PASSWORD, "--"])
        .arg(archive)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW.0)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| archive::parse_listing(&String::from_utf8_lossy(&output.stdout)))
}

/// Opens a folder in Explorer, or for a file, the folder it is in with the
/// file selected.
fn show(path: &Path) {
    let name = HSTRING::from(path.as_os_str());
    if path.is_dir() {
        unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                &name,
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        return;
    }
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let mut item: *mut ITEMIDLIST = std::ptr::null_mut();
        if SHParseDisplayName(&name, None, &mut item, 0, None).is_ok() {
            let _ = SHOpenFolderAndSelectItems(item, None, 0);
            CoTaskMemFree(Some(item as *const _));
        }
    }
}
