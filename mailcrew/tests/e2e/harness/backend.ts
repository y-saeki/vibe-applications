// The fake Rust side of mailcrew, injected into the page before the app's own
// bundle runs. `mockIPC` from the official Tauri API replaces
// `window.__TAURI_INTERNALS__.invoke`, so `src/` is loaded unmodified and every
// `invoke` it makes ends up in `handle` below.
//
// Built to tests/e2e/harness/dist/backend.js by tests/e2e/global-setup.ts.

import { mockIPC } from '@tauri-apps/api/mocks'

import type { Card, CommandError, Label } from '../../../src/backend'

/** What a test asks the fake backend to start from. */
export interface BackendConfig {
  /** The signed-in address; null starts signed out. */
  email: string | null
  platform: string
  /** What each `list_inbox` returns, in turn; the last one repeats. */
  inboxes: Card[][]
  labels: Label[]
  /** Commands that reject, with the error they reject with. */
  failures: Record<string, CommandError>
  /** What `sign_in` resolves with. */
  signInEmail: string
}

/** One `invoke` the app made, in the order it was made. */
export interface Call {
  cmd: string
  args: unknown
}

export interface Harness {
  config: BackendConfig
  calls: Call[]
}

const harness = window.__mailcrew
const { config } = harness
let inboxIndex = 0

function handle(cmd: string, args: unknown): unknown {
  harness.calls.push({ cmd, args })
  const failure = config.failures[cmd]
  if (failure) throw failure
  switch (cmd) {
    case 'get_session':
      return { email: config.email, platform: config.platform, version: '0.0.0-test' }
    case 'sign_in':
      return config.signInEmail
    case 'cancel_sign_in':
    case 'sign_out':
      return null
    case 'list_inbox': {
      const inbox = config.inboxes[Math.min(inboxIndex, config.inboxes.length - 1)] ?? []
      inboxIndex += 1
      return inbox
    }
    case 'list_labels':
      return config.labels
    case 'create_label': {
      const { name } = args as { name: string }
      return { id: `Label_${name}`, name }
    }
    case 'archive':
    case 'apply_label':
    case 'set_aside':
    case 'set_starred':
    case 'open_link':
      return null
    default:
      throw new Error(`mailcrew test: unexpected command ${cmd}`)
  }
}

mockIPC(handle)
