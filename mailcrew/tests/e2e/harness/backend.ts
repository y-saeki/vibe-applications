// The fake Rust side of mailcrew, injected into the page before the app's own
// bundle runs. `mockIPC` from the official Tauri API replaces
// `window.__TAURI_INTERNALS__.invoke`, so `src/` is loaded unmodified and every
// `invoke` it makes ends up in `handle` below.
//
// Built to tests/e2e/harness/dist/backend.js by tests/e2e/global-setup.ts.

import { mockIPC } from '@tauri-apps/api/mocks'

import type { AccountFailure, Card, CommandError, Label, View } from '../../../src/backend'

/** What a test asks the fake backend to start from. */
export interface BackendConfig {
  /** The signed-in addresses; empty starts signed out. */
  accounts: string[]
  platform: string
  /** The cards each `list_inbox` returns, in turn; the last one repeats. */
  inboxes: Card[][]
  /**
   * The accounts every `list_inbox` reports as failed. One that failed as
   * `signedOut` is removed, as the Rust side does.
   */
  inboxFailures: AccountFailure[]
  labels: Label[]
  /** The views saved when the app starts. */
  views: View[]
  /** Commands that reject, with the error they reject with. */
  failures: Record<string, CommandError>
  /** What `sign_in` resolves with, and adds to the accounts. */
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
let accounts = [...config.accounts]
let views = [...config.views]

function handle(cmd: string, args: unknown): unknown {
  harness.calls.push({ cmd, args })
  const failure = config.failures[cmd]
  if (failure) throw failure
  switch (cmd) {
    case 'get_session':
      return { accounts, platform: config.platform, version: '0.0.0-test' }
    case 'sign_in':
      if (!accounts.includes(config.signInEmail)) accounts = [...accounts, config.signInEmail]
      return config.signInEmail
    case 'cancel_sign_in':
      return null
    case 'sign_out': {
      const { account } = args as { account: string }
      accounts = accounts.filter((known) => known !== account)
      return null
    }
    case 'list_inbox': {
      if (accounts.length === 0) throw { kind: 'signedOut', message: 'Google アカウントにログインしていません' }
      const cards = config.inboxes[Math.min(inboxIndex, config.inboxes.length - 1)] ?? []
      inboxIndex += 1
      const failures = config.inboxFailures.filter((failure) => accounts.includes(failure.account))
      const gone = failures.filter((failure) => failure.kind === 'signedOut').map((failure) => failure.account)
      accounts = accounts.filter((account) => !gone.includes(account))
      return { cards, failures }
    }
    case 'list_views':
      return views
    case 'save_views':
      views = (args as { views: View[] }).views
      return null
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
