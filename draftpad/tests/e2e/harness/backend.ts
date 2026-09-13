// The fake Rust side of draftpad, injected into the page before the app's own
// bundle runs. `mockIPC` from the official Tauri API replaces
// `window.__TAURI_INTERNALS__.invoke`, so `src/` is loaded unmodified and every
// `invoke` it makes ends up in `handle` below.
//
// Built to tests/e2e/harness/dist/backend.js by tests/e2e/global-setup.ts.

import { mockIPC, mockWindows } from '@tauri-apps/api/mocks'

/** Mirrors `State` in src/state.ts. */
export interface State {
  text: string
  language: string
  editorMode: string
  theme: string
  fontSize: number
  fontFamily: string
  tabSize: number
  quickSuggestions: boolean
  alwaysOnTop: boolean
  windowWidth: number | null
  windowHeight: number | null
}

/** What a test asks the fake backend to start from. */
export interface BackendConfig {
  state?: Partial<State>
  /** `std::env::consts::OS` on the real side. */
  platform: string
  version: string
  openPreferences: boolean
  fonts: string[]
  /** Makes `load_state` reject, as a failing state file would. */
  failLoad?: string
  /** Makes `save_state` reject. */
  failSave?: string
  innerSize: { width: number; height: number }
  scaleFactor: number
}

/** One `invoke` the app made, in the order it was made. */
export interface Call {
  cmd: string
  args: unknown
}

export interface Harness {
  config: BackendConfig
  calls: Call[]
  /** The state the last `save_state` was given, or null before the first save. */
  saved: State | null
}

// Matches `impl Default for State` in src-tauri/src/state.rs.
const DEFAULT_STATE: State = {
  text: '',
  language: 'markdown',
  editorMode: 'normal',
  theme: 'system',
  fontSize: 13,
  fontFamily: '',
  tabSize: 4,
  quickSuggestions: true,
  alwaysOnTop: false,
  windowWidth: null,
  windowHeight: null,
}

const harness = window.__draftpad
const { config } = harness
const state: State = { ...DEFAULT_STATE, ...config.state }
let fullscreen = false

function handle(cmd: string, args: unknown): unknown {
  harness.calls.push({ cmd, args })
  switch (cmd) {
    case 'load_state':
      if (config.failLoad) throw new Error(config.failLoad)
      return {
        state,
        platform: config.platform,
        version: config.version,
        openPreferences: config.openPreferences,
      }
    case 'save_state': {
      if (config.failSave) throw new Error(config.failSave)
      harness.saved = (args as { state: State }).state
      return null
    }
    case 'list_fonts':
      return config.fonts
    case 'quit_app':
      return null
    // The window API the app reaches for, all of it plain IPC.
    case 'plugin:window|show':
    case 'plugin:window|destroy':
    case 'plugin:window|set_always_on_top':
      return null
    case 'plugin:window|set_fullscreen':
      fullscreen = (args as { value: boolean }).value
      return null
    case 'plugin:window|is_fullscreen':
      return fullscreen
    // draftpad never maximizes the window itself; it only asks before it
    // records a new size.
    case 'plugin:window|is_maximized':
      return false
    case 'plugin:window|inner_size':
      return config.innerSize
    case 'plugin:window|scale_factor':
      return config.scaleFactor
    default:
      throw new Error(`draftpad test: unexpected command ${cmd}`)
  }
}

mockWindows('main')
// `shouldMockEvents` makes the mock serve `plugin:event|listen` and
// `plugin:event|emit` itself, which is how a test delivers a "menu" event.
mockIPC(handle, { shouldMockEvents: true })
