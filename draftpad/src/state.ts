// Persistent state: the draft text plus every setting. Mirrors `State` in
// src-tauri/src/state.rs. Settings changes go through `Store.set`; the text
// lives in the editor and is pulled in lazily when a save happens.

import { invoke } from '@tauri-apps/api/core'

export type EditorMode = 'normal' | 'vim'
export type Theme = 'system' | 'light' | 'dark'

export interface State {
  text: string
  language: string
  editorMode: EditorMode
  theme: Theme
  fontSize: number
  fontFamily: string
  tabSize: number
  quickSuggestions: boolean
  alwaysOnTop: boolean
  windowWidth: number | null
  windowHeight: number | null
}

export interface Loaded {
  state: State
  /** `std::env::consts::OS` on the Rust side: "macos", "windows", "linux", ... */
  platform: string
  version: string
}

export const FONT_SIZE_MIN = 10
export const FONT_SIZE_MAX = 100
export const TAB_SIZE_MIN = 1
export const TAB_SIZE_MAX = 10

const SAVE_DEBOUNCE_MS = 300

export function loadState(): Promise<Loaded> {
  return invoke<Loaded>('load_state')
}

export type StateKey = keyof State
export type SettingsKey = Exclude<StateKey, 'text'>
export type SettingsPatch = Partial<Pick<State, SettingsKey>>
export type Listener = (state: Readonly<State>, changed: ReadonlySet<StateKey>) => void

export class Store {
  private current: State
  private textProvider: () => string
  private readonly listeners = new Set<Listener>()
  private timer: number | undefined
  private dirty = false
  private inflight: Promise<void> | null = null

  constructor(initial: State) {
    this.current = { ...initial }
    this.textProvider = () => this.current.text
  }

  get state(): Readonly<State> {
    return this.current
  }

  /** The editor registers itself here so saves read the live document. */
  setTextProvider(provider: () => string): void {
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

  /** Called on every document change; the text itself is read at save time. */
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
    this.current.text = this.textProvider()
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
