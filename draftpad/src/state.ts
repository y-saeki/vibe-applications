// Persistent state: the draft text plus every setting. Mirrors `State` in
// src-tauri/src/state.rs. Settings changes go through `Store.set`; the texts
// live in the editor and are pulled in lazily when a save happens.

import { invoke } from '@tauri-apps/api/core'

export type EditorMode = 'normal' | 'vim'
export type Theme = 'system' | 'light' | 'dark'
/** How a differing chunk is marked: whole lines only, or the characters within them too. */
export type DiffMode = 'line' | 'char'

export interface State {
  /** The draft: the only pane, or the left one while the compare pane is open. */
  text: string
  /** The right pane, while the compare pane is open; empty otherwise. */
  compareText: string
  /** Whether the compare pane is open, so the window holds two panes. */
  compare: boolean
  diffMode: DiffMode
  language: string
  editorMode: EditorMode
  theme: Theme
  fontSize: number
  fontFamily: string
  fontWeight: number
  tabSize: number
  quickSuggestions: boolean
  /** Whether the spaces and tabs in the draft carry a mark. */
  showWhitespace: boolean
  /** Whether each level of indentation carries a rule. */
  showIndentGuides: boolean
  /** Whether the draft carries a column of line numbers beside it. */
  showLineNumbers: boolean
  alwaysOnTop: boolean
  searchCaseSensitive: boolean
  searchRegexp: boolean
  windowWidth: number | null
  windowHeight: number | null
}

export interface Loaded {
  state: State
  /** `std::env::consts::OS` on the Rust side: "macos", "windows", "linux", ... */
  platform: string
  version: string
  /** True when this process was started by the Windows jump list "設定" task. */
  openPreferences: boolean
  /**
   * macOS: how far down the window the middle of its traffic lights is, in
   * CSS pixels. Null elsewhere, and when the Rust side could not tell.
   */
  trafficLightsCenter: number | null
}

/** The two texts, as the editor hands them back at save time. */
export interface Texts {
  text: string
  compareText: string
}

export const FONT_SIZE_MIN = 10
export const FONT_SIZE_MAX = 100
/** The nine weights CSS names, which is the scale the panel offers. */
export const FONT_WEIGHT_MIN = 100
export const FONT_WEIGHT_MAX = 900
export const FONT_WEIGHT_STEP = 100
export const TAB_SIZE_MIN = 1
export const TAB_SIZE_MAX = 10

const SAVE_DEBOUNCE_MS = 300

export function loadState(): Promise<Loaded> {
  return invoke<Loaded>('load_state')
}

export type StateKey = keyof State
export type TextKey = keyof Texts
export type SettingsKey = Exclude<StateKey, TextKey>
export type SettingsPatch = Partial<Pick<State, SettingsKey>>
export type Listener = (state: Readonly<State>, changed: ReadonlySet<StateKey>) => void

export class Store {
  private current: State
  private textProvider: () => Texts
  private readonly listeners = new Set<Listener>()
  private timer: number | undefined
  private dirty = false
  private inflight: Promise<void> | null = null

  constructor(initial: State) {
    this.current = { ...initial }
    this.textProvider = () => ({ text: this.current.text, compareText: this.current.compareText })
  }

  get state(): Readonly<State> {
    return this.current
  }

  /** The editor registers itself here so saves read the live documents. */
  setTextProvider(provider: () => Texts): void {
    this.textProvider = provider
  }

  subscribe(listener: Listener): () => void {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  /** Applies a settings patch, notifies listeners about keys that changed, and schedules a save. */
  set(patch: SettingsPatch): void {
    const changed = new Set<StateKey>()
    for (const key of Object.keys(patch) as SettingsKey[]) {
      const value = patch[key]
      if (value !== undefined && value !== this.current[key]) {
        ;(this.current as Record<SettingsKey, unknown>)[key] = value
        changed.add(key)
      }
    }
    if (changed.size === 0) return
    for (const listener of this.listeners) listener(this.current, changed)
    this.scheduleSave()
  }

  /** Called on every document change; the texts themselves are read at save time. */
  markTextChanged(): void {
    this.scheduleSave()
  }

  /** Cancels the pending debounce and writes now. Resolves once the file is written. */
  async flush(): Promise<void> {
    if (this.timer !== undefined) {
      window.clearTimeout(this.timer)
      this.timer = undefined
    }
    await this.save()
  }

  private scheduleSave(): void {
    this.dirty = true
    if (this.timer !== undefined) window.clearTimeout(this.timer)
    this.timer = window.setTimeout(() => {
      this.timer = undefined
      void this.save()
    }, SAVE_DEBOUNCE_MS)
  }

  private async save(): Promise<void> {
    // Serialize writes: wait for the one in flight, then write again if anything changed meanwhile.
    if (this.inflight) await this.inflight
    if (!this.dirty) return
    this.dirty = false
    const { text, compareText } = this.textProvider()
    this.current.text = text
    this.current.compareText = compareText
    const snapshot: State = { ...this.current }
    this.inflight = invoke<void>('save_state', { state: snapshot })
      .catch((err: unknown) => {
        console.error('draftpad: failed to save state', err)
        this.dirty = true
      })
      .finally(() => {
        this.inflight = null
      })
    await this.inflight
  }
}

export function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min
  return Math.min(max, Math.max(min, Math.round(value)))
}

/**
 * Snaps a weight onto one of the steps the panel offers, so that a value a
 * hand-edited file holds still lines up with an entry in its list.
 */
export function nearestFontWeight(value: number): number {
  const weight = clamp(value, FONT_WEIGHT_MIN, FONT_WEIGHT_MAX)
  return Math.round(weight / FONT_WEIGHT_STEP) * FONT_WEIGHT_STEP
}
