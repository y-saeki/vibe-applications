// Preferences panel. Every control writes straight into the Store; the app
// reacts to the resulting change notifications.

import { invoke } from '@tauri-apps/api/core'

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
  private readonly tabSize: HTMLInputElement
  private readonly quickSuggestions: HTMLInputElement
  private fontsLoaded = false

  constructor(
    private readonly root: HTMLElement,
    private readonly store: Store,
    private readonly options: PreferencesOptions,
  ) {
    const q = <T extends Element>(selector: string) => root.querySelector(selector) as T
    this.mode = q('#pref-mode')
    this.theme = q('#pref-theme')
    this.fontSize = q('#pref-font-size')
    this.fontFamily = q('#pref-font-family')
    this.fontList = q('#font-list')
    this.tabSize = q('#pref-tab-size')
    this.quickSuggestions = q('#pref-quick-suggestions')
    q<HTMLElement>('#pref-version').textContent = `draftpad ${options.version}`
    this.fontFamily.placeholder = options.defaultFontFamily

    this.mode.addEventListener('change', () => store.set({ editorMode: this.mode.value as EditorMode }))
    this.theme.addEventListener('change', () => store.set({ theme: this.theme.value as Theme }))
    this.fontSize.addEventListener('change', () => {
      const fontSize = clamp(Number(this.fontSize.value), FONT_SIZE_MIN, FONT_SIZE_MAX)
      this.fontSize.value = String(fontSize)
      store.set({ fontSize })
    })
    this.fontFamily.addEventListener('change', () => store.set({ fontFamily: this.fontFamily.value.trim() }))
    this.tabSize.addEventListener('change', () => {
      const tabSize = clamp(Number(this.tabSize.value), TAB_SIZE_MIN, TAB_SIZE_MAX)
      this.tabSize.value = String(tabSize)
      store.set({ tabSize })
    })
    this.quickSuggestions.addEventListener('change', () => store.set({ quickSuggestions: this.quickSuggestions.checked }))

    q<HTMLButtonElement>('#preferences-close').addEventListener('click', () => this.close())
    root.addEventListener('mousedown', (event) => {
      if (event.target === root) this.close()
    })
    store.subscribe(() => {
      if (!this.root.hidden) this.sync()
    })
  }

  get isOpen(): boolean {
    return !this.root.hidden
  }

  open(): void {
    this.sync()
    this.root.hidden = false
    this.focus()
    void this.loadFonts()
  }

  /** Puts the caret back in the panel, for when the window regains it. */
  focus(): void {
    this.mode.focus()
  }

  close(): void {
    if (this.root.hidden) return
    this.root.hidden = true
    this.options.onClose()
  }

  private sync(): void {
    const { state } = this.store
    this.mode.value = state.editorMode
    this.theme.value = state.theme
    this.fontSize.value = String(state.fontSize)
    if (document.activeElement !== this.fontFamily) this.fontFamily.value = state.fontFamily
    this.tabSize.value = String(state.tabSize)
    this.quickSuggestions.checked = state.quickSuggestions
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
