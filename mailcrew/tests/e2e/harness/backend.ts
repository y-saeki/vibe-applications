// The fake Rust side of mailcrew, injected into the page before the app's own
// bundle runs. `mockIPC` from the official Tauri API replaces
// `window.__TAURI_INTERNALS__.invoke`, so `src/` is loaded unmodified and every
// `invoke` it makes ends up in `handle` below.
//
// Built to tests/e2e/harness/dist/backend.js by tests/e2e/global-setup.ts.

import { emit } from '@tauri-apps/api/event'
import { mockIPC } from '@tauri-apps/api/mocks'

import type {
  AccountFailure,
  Brand,
  Card,
  CommandError,
  Label,
  MessageContent,
  Preferences,
  View,
} from '../../../src/backend'

/**
 * A card together with the body `get_message` hands over for it, and the
 * brand `get_brand` does.
 */
export type TestCard = Card & MessageContent & { brand: Brand }

/** What a test asks the fake backend to start from. */
export interface BackendConfig {
  /** The signed-in addresses; empty starts signed out. */
  accounts: string[]
  platform: string
  /** The cards each `list_inbox` returns, in turn; the last one repeats. */
  inboxes: TestCard[][]
  /**
   * The accounts every `list_inbox` reports as failed. One that failed as
   * `signedOut` is removed, as the Rust side does.
   */
  inboxFailures: AccountFailure[]
  labels: Label[]
  /** The views saved when the app starts. */
  views: View[]
  /** The senders whose images show without asking, when the app starts. */
  imageSenders: string[]
  /** The preferences saved when the app starts. */
  preferences: Preferences
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
  /** Sends an event as the Rust side would, such as `trackpad` from AppKit. */
  emit?: (event: string, payload: unknown) => Promise<void>
}

const harness = window.__mailcrew
const { config } = harness
let inboxIndex = 0
let accounts = [...config.accounts]
let views = [...config.views]
let imageSenders = [...config.imageSenders]
let preferences = { ...config.preferences }

function findMessage(args: unknown): TestCard {
  const { account, messageId } = args as { account: string; messageId: string }
  const found = config.inboxes.flat().find((card) => card.account === account && card.messageId === messageId)
  if (!found) throw new Error(`mailcrew test: no message ${messageId} in ${account}`)
  return found
}

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
      // The bodies come only through `get_message`, as from the Rust side.
      return {
        cards: cards.map(
          ({ body: _body, attachments: _attachments, authentication: _authentication, brand: _brand, ...card }) => card,
        ),
        failures,
      }
    }
    case 'get_message': {
      const found = findMessage(args)
      return { body: found.body, attachments: found.attachments, authentication: found.authentication }
    }
    case 'open_attachment':
      return null
    case 'save_attachment': {
      // The name it was saved under, as the Rust side resolves with.
      const { attachmentId } = args as { attachmentId: string }
      const found = findMessage(args).attachments.find((attachment) => attachment.id === attachmentId)
      if (!found) throw new Error(`mailcrew test: no attachment ${attachmentId}`)
      return found.name
    }
    case 'get_brand':
      // The Rust side looks nothing up while the preference is off.
      return preferences.brandLogos ? findMessage(args).brand : null
    case 'get_preferences':
      return preferences
    case 'save_preferences':
      preferences = (args as { preferences: Preferences }).preferences
      return null
    case 'list_views':
      return views
    case 'save_views':
      views = (args as { views: View[] }).views
      return null
    case 'list_image_senders':
      return imageSenders
    case 'save_image_senders': {
      // Lower case and each once, as the Rust side saves them.
      const given = (args as { senders: string[] }).senders.map((sender) => sender.trim().toLowerCase())
      imageSenders = [...new Set(given)].filter(Boolean)
      return imageSenders
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

// Events are mocked too, so a spec can send what the Rust side would.
mockIPC(handle, { shouldMockEvents: true })
harness.emit = (event, payload) => emit(event, payload)
