// The Rust side, as the page sees it. Every command is in
// src-tauri/src/commands.rs; the types mirror the structs there.

import { invoke } from '@tauri-apps/api/core'

export interface Session {
  email: string | null
  platform: string
  version: string
}

export interface Person {
  name: string
  email: string
}

export type CardBody =
  | { kind: 'html'; html: string; remoteImages: boolean }
  | { kind: 'text'; text: string }
  | { kind: 'empty' }

/** One thread in the inbox. Mirrors `Card` in src-tauri/src/card.rs. */
export interface Card {
  id: string
  subject: string
  from: Person
  /** Epoch milliseconds. */
  date: number
  messageCount: number
  unread: boolean
  starred: boolean
  messageId: string
  messageIds: string[]
  snippet: string
  body: CardBody
  attachments: string[]
}

export interface Label {
  id: string
  name: string
}

export type ErrorKind = 'signedOut' | 'cancelled' | 'network' | 'google' | 'keychain' | 'internal'

export interface CommandError {
  kind: ErrorKind
  message: string
}

export function isCommandError(err: unknown): err is CommandError {
  return typeof err === 'object' && err !== null && 'kind' in err && 'message' in err
}

export function errorMessage(err: unknown): string {
  if (isCommandError(err)) return err.message
  return err instanceof Error ? err.message : String(err)
}

export const backend = {
  session: () => invoke<Session>('get_session'),
  signIn: (clientId: string, clientSecret: string) => invoke<string>('sign_in', { clientId, clientSecret }),
  cancelSignIn: () => invoke<void>('cancel_sign_in'),
  signOut: () => invoke<void>('sign_out'),
  inbox: () => invoke<Card[]>('list_inbox'),
  labels: () => invoke<Label[]>('list_labels'),
  createLabel: (name: string) => invoke<Label>('create_label', { name }),
  archive: (messageIds: string[]) => invoke<void>('archive', { messageIds }),
  applyLabel: (messageIds: string[], labelId: string) => invoke<void>('apply_label', { messageIds, labelId }),
  setAside: (messageIds: string[]) => invoke<void>('set_aside', { messageIds }),
  setStarred: (messageId: string, messageIds: string[], starred: boolean) =>
    invoke<void>('set_starred', { messageId, messageIds, starred }),
  openLink: (url: string) => invoke<void>('open_link', { url }),
}
