// Preferences panel. Every control writes straight into the Store; the app
// reacts to the resulting change notifications.

import { invoke } from '@tauri-apps/api/core'

import { overlayScrollbar } from './overlay-scrollbar'
import { ShortcutList } from './shortcut-list'

import {
  clamp,
  FONT_SIZE_MAX,
  FONT_SIZE_MIN,
  TAB_SIZE_MAX,
  TAB_SIZE_MIN,
  type IndentStyle,
  type SettingsKey,
  type SettingsPatch,
  type State,
  type Store,
  type Theme,
} from './state'

export interface PreferencesOptions {
  version: string
  defaultFontFamily: string
  platform: string
  onClose: () => void
  /** Told when the shortcut tab starts and stops waiting for a key; see src/shortcut-list.ts. */
  onRecordingChange: (recording: boolean) => void
}

type Tab = 'general' | 'shortcuts'

/** Writes the store's value back into one control. */
type Sync = (state: Readonly<State>) => void

/** The settings whose value is of type `T`. */
type KeyOf<T> = { [K in SettingsKey]: State[K] extends T ? K : never }[SettingsKey]

/**
 * A dropdown: the option picked goes into the store as `parse` reads it.
 *
 * @returns what puts the store's value back into it
 */
function bindSelect<K extends SettingsKey>(store: Store, element: HTMLSelectElement, key: K, parse: (value: string) => State[K]): Sync {
  element.addEventListener('change', () => store.set({ [key]: parse(element.value) } as SettingsPatch))
  return (state) => {
    element.value = String(state[key])
  }
}

/**
 * A number field, held to `min`–`max`. A value outside that range is pulled
 * into it in the field as well, so that what it shows is what was taken.
 *
 * @returns what puts the store's value back into it
 */
function bindNumber(store: Store, element: HTMLInputElement, key: KeyOf<number>, min: number, max: number): Sync {
  element.addEventListener('change', () => {
    const value = clamp(Number(element.value), min, max)
    element.value = String(value)
    store.set({ [key]: value } as SettingsPatch)
  })
  return (state) => {
    element.value = String(state[key])
  }
}

/**
 * A checkbox.
 *
 * @returns what puts the store's value back into it
 */
function bindCheckbox(store: Store, element: HTMLInputElement, key: KeyOf<boolean>): Sync {
  element.addEventListener('change', () => store.set({ [key]: element.checked } as SettingsPatch))
  return (state) => {
    element.checked = state[key]
  }
}

export class Preferences {
  /** The general tab's first control, where the caret goes when it opens on that tab. */
  private readonly first: HTMLSelectElement
  private readonly filter: HTMLInputElement
  private readonly shortcuts: ShortcutList
  private readonly tabs: Record<Tab, { button: HTMLButtonElement; panel: HTMLElement }>
  /** The tab the panel opens on: the one it was last left on. */
  private tab: Tab = 'general'
  private readonly fontList: HTMLDataListElement
  /** One per control, in the order the panel holds them. */
  private readonly syncs: Sync[]
  private fontsLoaded = false

  constructor(
    private readonly root: HTMLDialogElement,
    private readonly store: Store,
    options: PreferencesOptions,
  ) {
    const q = <T extends Element>(selector: string) => root.querySelector(selector) as T
    this.first = q('#pref-indent-style')
    this.fontList = q('#font-list')
    q<HTMLElement>('#pref-version').textContent = `draftpad ${options.version}`
    // The panel's body scrolls once the window is too short to hold it; the
    // heading and the tabs above it stay. Its bar goes inside the dialog: it is
    // in the top layer, and nothing outside it is drawn over it. The body is
    // as tall as the window allows whatever it holds, so the bar also watches
    // what is inside it, which changes with the tab and the filter.
    overlayScrollbar(q<HTMLElement>('.panel-body'), root, q<HTMLElement>('.panel-content'))

    const fontFamily = q<HTMLInputElement>('#pref-font-family')
    fontFamily.placeholder = options.defaultFontFamily
    fontFamily.addEventListener('change', () => store.set({ fontFamily: fontFamily.value.trim() }))

    this.syncs = [
      bindSelect(store, this.first, 'indentStyle', (value) => value as IndentStyle),
      bindNumber(store, q('#pref-tab-size'), 'tabSize', TAB_SIZE_MIN, TAB_SIZE_MAX),
      bindSelect(store, q('#pref-theme'), 'theme', (value) => value as Theme),
      // Left alone while it has the keyboard, so that what is being typed is
      // not overwritten by what was there before.
      (state) => {
        if (document.activeElement !== fontFamily) fontFamily.value = state.fontFamily
      },
      bindSelect(store, q('#pref-font-weight'), 'fontWeight', Number),
      bindNumber(store, q('#pref-font-size'), 'fontSize', FONT_SIZE_MIN, FONT_SIZE_MAX),
      bindCheckbox(store, q('#pref-show-whitespace'), 'showWhitespace'),
      bindCheckbox(store, q('#pref-show-indent-guides'), 'showIndentGuides'),
      bindCheckbox(store, q('#pref-show-line-numbers'), 'showLineNumbers'),
    ]

    this.filter = q('#shortcut-filter')
    this.shortcuts = new ShortcutList(q('#shortcut-list'), this.filter, q('#shortcut-count'), q('#shortcut-reset-all'), store, {
      platform: options.platform,
      onRecordingChange: options.onRecordingChange,
    })
    this.tabs = {
      general: { button: q('#pref-tab-general'), panel: q('#pref-general') },
      shortcuts: { button: q('#pref-tab-shortcuts'), panel: q('#pref-shortcuts') },
    }
    for (const tab of ['general', 'shortcuts'] as const) {
      const { button } = this.tabs[tab]
      button.addEventListener('click', () => this.showTab(tab))
      // The tab pattern: one stop for the pair, and the arrows move between them.
      button.addEventListener('keydown', (event) => {
        if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
        event.preventDefault()
        const other = tab === 'general' ? 'shortcuts' : 'general'
        this.showTab(other)
        this.tabs[other].button.focus()
      })
    }
    // A key being waited for is the list's, whatever it is — including the
    // ones the window acts on (src/main.ts listens after this) and Escape,
    // which would otherwise close the dialog.
    window.addEventListener(
      'keydown',
      (event) => {
        if (!this.isOpen || !this.shortcuts.handleKey(event)) return
        event.stopImmediatePropagation()
      },
      true,
    )
    root.addEventListener('cancel', (event) => {
      if (this.shortcuts.isRecording) event.preventDefault()
    })

    q<HTMLButtonElement>('#preferences-close').addEventListener('click', () => this.close())
    // The dialog fills the window and draws the dim itself, so anything outside
    // the panel is a click on it rather than on a child.
    root.addEventListener('mousedown', (event) => {
      if (event.target === root) this.close()
    })
    // Escape closes the dialog without going through close(), so the hand-back
    // hangs off the event every path ends at.
    root.addEventListener('close', () => {
      this.shortcuts.reset()
      options.onClose()
    })
    store.subscribe(() => {
      if (this.isOpen) this.sync()
    })
  }

  get isOpen(): boolean {
    return this.root.open
  }

  open(): void {
    this.showTab(this.tab)
    this.sync()
    // Modal rather than plain open(): the top layer puts the panel over the
    // editor's own chrome, and the rest of the window stops taking input.
    this.root.showModal()
    this.focus()
    void this.loadFonts()
  }

  /** Puts the caret back in the panel, for when the window regains it. */
  focus(): void {
    if (this.tab === 'general') this.first.focus()
    else this.filter.focus()
  }

  private showTab(tab: Tab): void {
    this.tab = tab
    for (const [name, { button, panel }] of Object.entries(this.tabs)) {
      const selected = name === tab
      button.setAttribute('aria-selected', String(selected))
      button.tabIndex = selected ? 0 : -1
      panel.hidden = !selected
    }
    this.root.dataset.tab = tab
    if (tab === 'shortcuts') this.shortcuts.render()
  }

  close(): void {
    this.root.close()
  }

  private sync(): void {
    for (const sync of this.syncs) sync(this.store.state)
    if (this.tab === 'shortcuts') this.shortcuts.render()
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
