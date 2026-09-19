//! Turning off the WebView2 autofill (Windows only).
//!
//! WebView2 treats every text input as a form field, so focusing the font box
//! in the preferences panel pops up a "保存された情報" bubble offering entries
//! Edge saved elsewhere. diffpad has no forms, so the offer is only in the way.

use tauri::WebviewWindow;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2Controller, ICoreWebView2Settings4,
};
use windows_core::{Interface, Result};

/// Disables the autofill on `window`'s webview. The work happens on the
/// webview thread, so it may finish after this call returns.
pub fn disable(window: &WebviewWindow) {
    let dispatched = window.with_webview(|webview| {
        if let Err(err) = disable_on(&webview.controller()) {
            eprintln!("diffpad: failed to turn off the WebView2 autofill: {err}");
        }
    });
    if let Err(err) = dispatched {
        eprintln!("diffpad: failed to reach the WebView2 controller: {err}");
    }
}

fn disable_on(controller: &ICoreWebView2Controller) -> Result<()> {
    // ICoreWebView2Settings4 carries both switches; an older runtime that does
    // not implement it fails the cast and keeps its defaults.
    unsafe {
        let settings: ICoreWebView2Settings4 = controller.CoreWebView2()?.Settings()?.cast()?;
        settings.SetIsGeneralAutofillEnabled(false)?;
        settings.SetIsPasswordAutosaveEnabled(false)
    }
}
