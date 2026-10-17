//! Applies shell.rs's plans to HKEY_CURRENT_USER, and tells Explorer that
//! file associations changed.

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegDeleteTreeW, RegSetKeyValueW,
};
use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
use windows::core::HSTRING;

use crate::shell::{self, Plan};

/// Writes what eleven-zip needs for this executable, with the context menu's
/// verbs or without.
pub fn register(context_menu: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    apply(&shell::register(&exe.to_string_lossy(), context_menu))
}

pub fn unregister() -> Result<(), String> {
    apply(&shell::unregister())
}

fn apply(plan: &Plan) -> Result<(), String> {
    let result = (|| {
        for key in &plan.remove_keys {
            check(
                unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(key)) },
                key,
            )?;
        }
        for (key, name) in &plan.remove_values {
            check(
                unsafe {
                    RegDeleteKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(key), &HSTRING::from(name))
                },
                key,
            )?;
        }
        for v in &plan.set {
            let data: Vec<u8> = v
                .data
                .encode_utf16()
                .chain([0])
                .flat_map(u16::to_le_bytes)
                .collect();
            check(
                unsafe {
                    RegSetKeyValueW(
                        HKEY_CURRENT_USER,
                        &HSTRING::from(&v.key),
                        &HSTRING::from(&v.name),
                        REG_SZ.0,
                        Some(data.as_ptr().cast()),
                        data.len() as u32,
                    )
                },
                &v.key,
            )?;
        }
        Ok(())
    })();
    // Explorer caches associations and menus; this makes it read them again.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
    result
}

/// Something already gone counts as removed.
fn check(status: WIN32_ERROR, key: &str) -> Result<(), String> {
    if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(format!(
            "レジストリを変更できません: HKEY_CURRENT_USER\\{key} ({})",
            status.0
        ))
    }
}
