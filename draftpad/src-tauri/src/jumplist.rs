//! Windows taskbar jump list (Windows only).
//!
//! Right-clicking the taskbar button shows a "設定" task. Jump list tasks are
//! shortcuts, so the task can only start the executable again — it passes
//! [`PREFERENCES_ARG`], and `tauri-plugin-single-instance` (wired up in
//! `lib.rs`) hands that argument to the instance that is already running.
//! A cold start reads the same argument in `commands::load_state`.

use std::ffi::OsStr;
use std::iter::once;
use std::mem::ManuallyDrop;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::core::{Interface, Result, PCWSTR, PWSTR};
use windows::Win32::Foundation::E_OUTOFMEMORY;
use windows::Win32::Storage::EnhancedStorage::PKEY_Title;
use windows::Win32::System::Com::StructuredStorage::{
    PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemAlloc, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{
    DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW, ShellLink,
};

/// The argument the "設定" task passes to the process it starts.
pub const PREFERENCES_ARG: &str = "--preferences";

const TASK_TITLE: &str = "設定";
const TASK_DESCRIPTION: &str = "draftpad の環境設定を開きます";

/// True when this process was started by the jump list task.
pub fn started_for_preferences() -> bool {
    std::env::args().any(|arg| arg == PREFERENCES_ARG)
}

/// Publishes the jump list for this process.
///
/// The list is attached to the process' application user model id, which we
/// deliberately leave at the default derived from the executable path so the
/// taskbar button keeps grouping with an installed shortcut.
///
/// Shell COM wants a single-threaded apartment, and the list is rebuilt from
/// scratch on every launch anyway, so this runs on a scratch thread rather than
/// touching the apartment the event loop lives in.
pub fn install() {
    let Ok(exe) = std::env::current_exe() else {
        eprintln!("draftpad: cannot locate the executable; skipping the jump list");
        return;
    };
    std::thread::spawn(move || unsafe {
        // S_FALSE ("already initialised on this thread") is a success and still
        // needs the matching CoUninitialize, so only a hard error bails out.
        if CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_err() {
            eprintln!("draftpad: failed to initialize COM for the jump list");
            return;
        }
        if let Err(err) = build(&exe) {
            eprintln!("draftpad: failed to build the jump list: {err}");
        }
        CoUninitialize();
    });
}

unsafe fn build(exe: &Path) -> Result<()> {
    let list: ICustomDestinationList =
        CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
    // BeginList reports how many slots the taskbar will show and hands back the
    // destinations the user removed by hand. We add a single task and no
    // destinations, so neither is of any use to us.
    let mut slots = 0u32;
    let _removed: IObjectArray = list.BeginList(&mut slots)?;

    let tasks: IObjectCollection =
        CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
    tasks.AddObject(&preferences_task(exe)?)?;
    list.AddUserTasks(&tasks.cast::<IObjectArray>()?)?;
    list.CommitList()
}

/// The "設定" entry: this executable, started again with [`PREFERENCES_ARG`].
unsafe fn preferences_task(exe: &Path) -> Result<IShellLinkW> {
    let task: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
    let path = wide(exe);
    task.SetPath(PCWSTR(path.as_ptr()))?;
    task.SetIconLocation(PCWSTR(path.as_ptr()), 0)?;
    let args = wide(PREFERENCES_ARG);
    task.SetArguments(PCWSTR(args.as_ptr()))?;
    let description = wide(TASK_DESCRIPTION);
    task.SetDescription(PCWSTR(description.as_ptr()))?;

    // The label the taskbar draws does not come from the shortcut itself: it is
    // System.Title in the link's property store.
    let store: IPropertyStore = task.cast()?;
    store.SetValue(&PKEY_Title, &lpwstr(TASK_TITLE)?)?;
    store.Commit()?;
    Ok(task)
}

fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(once(0)).collect()
}

/// A `VT_LPWSTR` PROPVARIANT. The string has to come from the COM allocator
/// because `PropVariantClear` — which the returned value runs on drop — frees
/// it with `CoTaskMemFree`.
unsafe fn lpwstr(value: &str) -> Result<PROPVARIANT> {
    let text = wide(value);
    let buffer = CoTaskMemAlloc(std::mem::size_of_val(text.as_slice())) as *mut u16;
    if buffer.is_null() {
        return Err(windows::core::Error::from_hresult(E_OUTOFMEMORY));
    }
    std::ptr::copy_nonoverlapping(text.as_ptr(), buffer, text.len());
    Ok(PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                vt: VT_LPWSTR,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: PROPVARIANT_0_0_0 {
                    pwszVal: PWSTR(buffer),
                },
            }),
        },
    })
}
