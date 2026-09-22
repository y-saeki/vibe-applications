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
  compareText: string
  compare: boolean
  diffMode: string
  language: string
  editorMode: string
  theme: string
  fontSize: number
  fontFamily: string
  fontWeight: number
  tabSize: number
  quickSuggestions: boolean
  showWhitespace: boolean
  showIndentGuides: boolean
  showLineNumbers: boolean
  alwaysOnTop: boolean
  searchCaseSensitive: boolean
  searchRegexp: boolean
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
  /** What the Rust side reads off the macOS traffic lights; left out, it could not tell. */
  trafficLightsCenter?: number
  fonts: string[]
  /** Makes `load_state` reject, as a failing state file would. */
  failLoad?: string
  /** Makes `save_state` reject. */
  failSave?: string
  /** What the clipboard holds; left out, reading it fails as an empty one does. */
  clipboard?: string
  /** The window's inner size in physical pixels, which `set_size` moves on. */
  innerSize: { width: number; height: number }
  scaleFactor: number
}

/** One `invoke` the app made, in the order it was made. */
export interface Call {
  cmd: string
  args: unknown
}

/** A window size in logical pixels, as the app asks for one. */
export interface Resize {
  width: number
  height: number
}

export interface Harness {
  config: BackendConfig
  calls: Call[]
  /** The state the last `save_state` was given, or null before the first save. */
  saved: State | null
  /** The size the last `set_size` asked for, or null before the first. */
  resized: Resize | null
}

// Matches `impl Default for State` in src-tauri/src/state.rs.
const DEFAULT_STATE: State = {
  text: '',
  compareText: '',
  compare: false,
  diffMode: 'char',
  language: 'markdown',
  editorMode: 'normal',
  theme: 'system',
  fontSize: 13,
  fontFamily: '',
  fontWeight: 400,
  tabSize: 4,
  quickSuggestions: true,
  showWhitespace: false,
  showIndentGuides: false,
  showLineNumbers: false,
  alwaysOnTop: false,
  searchCaseSensitive: false,
  searchRegexp: false,
  windowWidth: null,
  windowHeight: null,
}

const harness = window.__draftpad
const { config } = harness
const state: State = { ...DEFAULT_STATE, ...config.state }
let fullscreen = false
let maximized = false

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
        trafficLightsCenter: config.trafficLightsCenter ?? null,
      }
    case 'save_state': {
      if (config.failSave) throw new Error(config.failSave)
      harness.saved = (args as { state: State }).state
      return null
    }
    case 'list_fonts':
      return config.fonts
    // The real plugin rejects when the clipboard holds no text at all, which
    // is what an unset `clipboard` stands for here.
    case 'plugin:clipboard-manager|read_text':
      if (config.clipboard === undefined) throw new Error('clipboard is empty')
      return config.clipboard
    case 'quit_app':
      return null
    // The native menu itself is outside the webview; what the page decides
    // before asking for it is not.
    case 'show_context_menu':
      return null
    // The window API the app reaches for, all of it plain IPC.
    case 'plugin:window|show':
    case 'plugin:window|destroy':
    case 'plugin:window|set_always_on_top':
    case 'plugin:window|minimize':
      return null
    // The Windows title bar's maximize button. What the page learns of the
    // result comes back the way the real window tells it: a resize.
    case 'plugin:window|toggle_maximize':
      maximized = !maximized
      return null
    case 'plugin:window|set_fullscreen':
      fullscreen = (args as { value: boolean }).value
      return null
    case 'plugin:window|is_fullscreen':
      return fullscreen
    // Asked before a new size is recorded or changed, and by the maximize
    // button, which follows the window.
    case 'plugin:window|is_maximized':
      return maximized
    case 'plugin:window|inner_size':
      return config.innerSize
    case 'plugin:window|scale_factor':
      return config.scaleFactor
    case 'plugin:window|set_size': {
      // The size arrives as the API's own `Size`, which goes on the wire as
      // `{ Logical: { width, height } }` or `{ Physical: ... }`. draftpad
      // only ever asks in logical pixels, which is what the real window
      // scales for the screen.
      const wire = JSON.parse(JSON.stringify((args as { value: unknown }).value)) as { Logical?: Resize }
      if (!wire.Logical) throw new Error('draftpad test: set_size was given a size that is not logical')
      harness.resized = wire.Logical
      // The window is that size from now on, so the next resize scales this one.
      config.innerSize = {
        width: wire.Logical.width * config.scaleFactor,
        height: wire.Logical.height * config.scaleFactor,
      }
      return null
    }
    default:
      throw new Error(`draftpad test: unexpected command ${cmd}`)
  }
}

mockWindows('main')
// `shouldMockEvents` makes the mock serve `plugin:event|listen` and
// `plugin:event|emit` itself, which is how a test delivers a "menu" event.
mockIPC(handle, { shouldMockEvents: true })
