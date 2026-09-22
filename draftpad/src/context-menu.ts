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

/** The input types holding text that cut, copy and paste can act on. */
const TEXT_INPUT_TYPES = new Set(['text', 'search', 'number', 'url', 'tel', 'email', 'password'])

/**
 * True where text can be edited: the draft, and the text fields in the search
 * and preferences panels. The rest of the window — the bars, a button, a
 * checkbox, a dropdown — has nothing for this menu to act on.
 */
function isEditable(target: EventTarget | null): boolean {
  if (target instanceof HTMLTextAreaElement) return true
  if (target instanceof HTMLInputElement) return TEXT_INPUT_TYPES.has(target.type)
  // True for anything inside the editor, not just its outermost element.
  return target instanceof HTMLElement && target.isContentEditable
}

/**
 * Replaces WKWebView's context menu with draftpad's.
 *
 * WKWebView's is stopped everywhere, including where draftpad opens none of
 * its own: a menu over the title bar would only offer commands with nothing
 * to act on.
 *
 * @param state what to leave enabled in the menu
 */
export function installContextMenu(state: () => ContextMenuState): void {
  window.addEventListener(
    'contextmenu',
    (event) => {
      event.preventDefault()
      if (!isEditable(event.target)) return
      // Deliberately not awaited: a native menu runs a modal loop, so this only
      // settles once the user has picked something or dismissed it.
      void invoke('show_context_menu', { state: state() }).catch((err: unknown) => {
        console.error('draftpad: opening the context menu failed', err)
      })
    },
    true,
  )
}
