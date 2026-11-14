// The fake Rust side of pecha-player, injected into the page before the app's
// own bundle runs. `mockIPC` from the official Tauri API replaces
// `window.__TAURI_INTERNALS__.invoke`, so `src/` is loaded unmodified and every
// `invoke` it makes ends up in `handle` below.
//
// Built to tests/e2e/harness/dist/backend.js by tests/e2e/global-setup.ts.

import { emit } from '@tauri-apps/api/event'
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks'

import { makePdf } from './pdf'

/** What `load_google_slides` answers for a URL: a deck of so many pages, or an error. */
export type GoogleAnswer = { pages: number } | { error: { kind: string; detail?: string } }

export interface BackendConfig {
  /**
   * Answers for `load_google_slides`, per URL, used in order; the last one
   * repeats. A URL with none is answered with `invalidUrl`.
   */
  google: Record<string, GoogleAnswer[]>
}

/** One `invoke` the app made, in the order it was made. */
export interface Call {
  cmd: string
  args: unknown
}

export interface Harness {
  config: BackendConfig
  calls: Call[]
  /** Sends a backend event to the page, as `Emitter::emit_to` would. */
  emit: (event: string, payload: unknown) => Promise<void>
  /** Puts the window in or out of fullscreen without the page asking, as the green button does. */
  fullscreenFromOutside: (on: boolean) => Promise<void>
}

const harness = window.__pecha
const { config } = harness
const asked = new Map<string, number>()
let fullscreen = false

function google(url: string): ArrayBuffer {
  const answers = config.google[url]
  if (!answers || answers.length === 0) throw { kind: 'invalidUrl' }
  const n = asked.get(url) ?? 0
  asked.set(url, n + 1)
  const answer = answers[Math.min(n, answers.length - 1)]!
  if ('error' in answer) throw answer.error
  return makePdf(answer.pages).buffer as ArrayBuffer
}

/** Resolves with `value` a few turns of the microtask queue from now. */
async function later<T>(value: T): Promise<T> {
  for (let i = 0; i < 10; i++) await Promise.resolve()
  return value
}

function handle(cmd: string, args: unknown): unknown {
  harness.calls.push({ cmd, args })
  switch (cmd) {
    case 'load_google_slides':
      return google((args as { url: string }).url)
    case 'open_google_sign_in':
      return null
    case 'plugin:window|set_fullscreen':
      fullscreen = (args as { value: boolean }).value
      // Like the real window, it resizes before the command has answered.
      void emit('tauri://resize', { width: 1710, height: 1112 })
      return null
    case 'set_slide_fullscreen':
      // On macOS the real window does not report this through is_fullscreen,
      // so the page must not rely on it. The window resizes before the
      // command has answered.
      void emit('tauri://resize', { width: 1710, height: 1112 })
      return null
    case 'plugin:window|is_fullscreen':
      // Answered late, as a round trip to the window is: after a command the
      // page sent at the same time.
      return later(fullscreen)
    case 'plugin:window|set_title':
      return null
    case 'set_slide_fullscreen':
      // On macOS the real window does not report this through is_fullscreen,
      // so the page must not rely on it. The window resizes before the
      // command has answered.
      void emit('tauri://resize', { width: 1710, height: 1112 })
      return null
    case 'plugin:window|is_fullscreen':
      // Answered late, as a round trip to the window is: after a command the
      // page sent at the same time.
      return later(fullscreen)
    default:
      throw new Error(`pecha-player test: unexpected command ${cmd}`)
  }
}

harness.emit = (event, payload) => emit(event, payload)
harness.fullscreenFromOutside = (on) => {
  fullscreen = on
  return emit('tauri://resize', { width: 1710, height: 1112 })
}
mockWindows('main')
mockIPC(handle, { shouldMockEvents: true })
