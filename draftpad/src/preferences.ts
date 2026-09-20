// Preferences panel. Every control writes straight into the Store; the app
// reacts to the resulting change notifications.

import { invoke } from '@tauri-apps/api/core'

import { overlayScrollbars } from './overlay-scrollbar'

import {
  clamp,
  FONT_SIZE_MAX,
  FONT_SIZE_MIN,
  TAB_SIZE_MAX,
  TAB_SIZE_MIN,
  type EditorMode,
  type Store,
  type Theme,
} from './state'

export interface PreferencesOptions {
  version: string
  defaultFontFamily: string
  onClose: () => void
}

export class Preferences {
  private readonly mode: HTMLSelectElement
  private readonly theme: HTMLSelectElement
  private readonly fontSize: HTMLInputElement
  private readonly fontFamily: HTMLInputElement
  private readonly fontList: HTMLDataListElement
  private readonly fontWeight: HTMLSelectElement
  private readonly tabSize: HTMLInputElement
  private readonly quickSuggestions: HTMLInputElement
  private readonly showWhitespace: HTMLInputElement
  private readonly showIndentGuides: HTMLInputElement
  private fontsLoaded = false

  constructor(
    private readonly root: HTMLDialogElement,
    private readonly store: Store,
    options: PreferencesOptions,
  ) {
    const q = <T extends Element>(selector: string) => root.querySelector(selector) as T
    this.mode = q('#pref-mode')
    this.theme = q('#pref-theme')
    this.fontSize = q('#pref-font-size')
    this.fontFamily = q('#pref-font-family')
    this.fontList = q('#font-list')
    this.fontWeight = q('#pref-font-weight')
    this.tabSize = q('#pref-tab-size')
    this.quickSuggestions = q('#pref-quick-suggestions')
    this.showWhitespace = q('#pref-show-whitespace')
    this.showIndentGuides = q('#pref-show-indent-guides')
    q<HTMLElement>('#pref-version').textContent = `draftpad ${options.version}`
    // The panel scrolls once the window is too short to hold it. Its bars go
    // inside the dialog: it is in the top layer, and nothing outside it is
    // drawn over it.
    overlayScrollbars(q<HTMLElement>('.panel'), root)
    this.fontFamily.placeholder = options.defaultFontFamily

    this.mode.addEventListener('change', () => store.set({ editorMode: this.mode.value as EditorMode }))
    this.theme.addEventListener('change', () => store.set({ theme: this.theme.value as Theme }))
    this.fontSize.addEventListener('change', () => {
      const fontSize = clamp(Number(this.fontSize.value), FONT_SIZE_MIN, FONT_SIZE_MAX)
      this.fontSize.value = String(fontSize)
      store.set({ fontSize })
    })
    this.fontFamily.addEventListener('change', () => store.set({ fontFamily: this.fontFamily.value.trim() }))
    this.fontWeight.addEventListener('change', () => store.set({ fontWeight: Number(this.fontWeight.value) }))
    this.tabSize.addEventListener('change', () => {
      const tabSize = clamp(Number(this.tabSize.value), TAB_SIZE_MIN, TAB_SIZE_MAX)
      this.tabSize.value = String(tabSize)
      store.set({ tabSize })
    })
    this.quickSuggestions.addEventListener('change', () => store.set({ quickSuggestions: this.quickSuggestions.checked }))
    this.showWhitespace.addEventListener('change', () => store.set({ showWhitespace: this.showWhitespace.checked }))
    this.showIndentGuides.addEventListener('change', () => store.set({ showIndentGuides: this.showIndentGuides.checked }))

    q<HTMLButtonElement>('#preferences-close').addEventListener('click', () => this.close())
    // The dialog fills the window and draws the dim itself, so anything outside
    // the panel is a click on it rather than on a child.
    root.addEventListener('mousedown', (event) => {
      if (event.target === root) this.close()
    })
    // Escape closes the dialog without going through close(), so the hand-back
    // hangs off the event every path ends at.
    root.addEventListener('close', () => options.onClose())
    store.subscribe(() => {
      if (this.isOpen) this.sync()
    })
  }

  get isOpen(): boolean {
    return this.root.open
  }

  open(): void {
    this.sync()
    // Modal rather than plain open(): the top layer puts the panel over the
    // editor's own chrome, and the rest of the window stops taking input.
    this.root.showModal()
    this.focus()
    void this.loadFonts()
  }

  /** Puts the caret back in the panel, for when the window regains it. */
  focus(): void {
    this.mode.focus()
  }

  close(): void {
    this.root.close()
  }

  private sync(): void {
    const { state } = this.store
    this.mode.value = state.editorMode
    this.theme.value = state.theme
    this.fontSize.value = String(state.fontSize)
    if (document.activeElement !== this.fontFamily) this.fontFamily.value = state.fontFamily
    this.fontWeight.value = String(state.fontWeight)
    this.tabSize.value = String(state.tabSize)
    this.quickSuggestions.checked = state.quickSuggestions
    this.showWhitespace.checked = state.showWhitespace
    this.showIndentGuides.checked = state.showIndentGuides
  }

  private async loadFonts(): Promise<void> {
    if (this.fontsLoaded) return
    this.fontsLoaded = true
    try {
      const families = await invoke<string[]>('list_fonts')
      this.fontList.replaceChildren(
        ...families.map((family) => {
          const option = document.createElement('option')
          option.value = family
          return option
        }),
      )
    } catch (err) {
      console.error('draftpad: failed to list fonts', err)
      this.fontsLoaded = false
    }
  }
}
