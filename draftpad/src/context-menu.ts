// The right-click menu on macOS. The page draws none of it: it only stops
// WKWebView from opening its own — a page of speech, font, substitution and
// writing-tools entries that have nothing to do with a draft — and asks the
// Rust side to put up a native one holding the few commands that do
// (src-tauri/src/menu.rs).
//
// Windows keeps the webview's own menu, which WebView2 lets `context_menu.rs`
// trim in place, so nothing here runs there.

import { invoke } from '@tauri-apps/api/core'

/** What the menu cannot work out for itself; read at the moment of the click. */
export interface ContextMenuState {
  canUndo: boolean
  canRedo: boolean
  canSearch: boolean
}

/**
 * Replaces WKWebView's context menu with draftpad's, everywhere in the window.
 *
 * @param state what to leave enabled in the menu
 */
export function installContextMenu(state: () => ContextMenuState): void {
  window.addEventListener(
    'contextmenu',
    (event) => {
      event.preventDefault()
      // Deliberately not awaited: a native menu runs a modal loop, so this only
      // settles once the user has picked something or dismissed it.
      void invoke('show_context_menu', { state: state() }).catch((err: unknown) => {
        console.error('draftpad: opening the context menu failed', err)
      })
    },
    true,
  )
}
