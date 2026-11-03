//! The system's dialogs that choose a file: one to export the settings to
//! or import them from, or an application's executable to add to a set.

use std::path::PathBuf;

use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FileOpenDialog, FileSaveDialog,
    IFileDialog, IFileOpenDialog, IFileSaveDialog, SIGDN_FILESYSPATH,
};
use windows::core::{HRESULT, Interface, w};

/// What the exported file is called unless the user names it otherwise.
const FILE_NAME: &str = "meta-gesture.toml";

/// What the dialog chooses.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Choose {
    /// Where to export the settings to.
    Export,
    /// Settings to import.
    Import,
    /// An application's executable.
    Executable,
}

/// Asks for a file over the window `owner` (an `HWND` as a number, so that
/// it can cross threads). `None` when the dialog was cancelled.
///
/// The dialog is modal and runs a message loop of its own, so it is shown
/// on a thread of its own rather than inside the settings window's
/// `update`, which that loop would otherwise enter again.
pub fn choose(owner: isize, what: Choose) -> Result<Option<PathBuf>, String> {
    std::thread::spawn(move || {
        let _com = Com::init();
        show(owner, what).map_err(|e| e.message())
    })
    .join()
    .unwrap_or_else(|_| Err("ファイルを選ぶダイアログを開けませんでした".into()))
}

fn show(owner: isize, what: Choose) -> windows::core::Result<Option<PathBuf>> {
    let save = what == Choose::Export;
    let dialog: IFileDialog = unsafe {
        if save {
            let d: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
            d.cast()?
        } else {
            let d: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            d.cast()?
        }
    };
    unsafe {
        let all = COMDLG_FILTERSPEC {
            pszName: w!("すべてのファイル (*.*)"),
            pszSpec: w!("*.*"),
        };
        if what == Choose::Executable {
            dialog.SetFileTypes(&[
                COMDLG_FILTERSPEC {
                    pszName: w!("アプリケーション (*.exe)"),
                    pszSpec: w!("*.exe"),
                },
                all,
            ])?;
        } else {
            dialog.SetFileTypes(&[
                COMDLG_FILTERSPEC {
                    pszName: w!("meta-gesture の設定 (*.toml)"),
                    pszSpec: w!("*.toml"),
                },
                all,
            ])?;
            dialog.SetDefaultExtension(w!("toml"))?;
        }
        let options = dialog.GetOptions()?
            | FOS_FORCEFILESYSTEM
            | if save {
                FOS_OVERWRITEPROMPT
            } else {
                FOS_FILEMUSTEXIST
            };
        dialog.SetOptions(options)?;
        if save {
            dialog.SetFileName(&windows::core::HSTRING::from(FILE_NAME))?;
        }
        let owner = (owner != 0).then_some(HWND(owner as *mut _));
        if let Err(e) = dialog.Show(owner) {
            if e.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
                return Ok(None);
            }
            return Err(e);
        }
        let name = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = String::from_utf16_lossy(name.as_wide());
        CoTaskMemFree(Some(name.0 as *const _));
        Ok(Some(PathBuf::from(path)))
    }
}

/// COM on the dialog's thread, for as long as the dialog is open.
struct Com(bool);

impl Com {
    fn init() -> Com {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        Com(hr.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}
