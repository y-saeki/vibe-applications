// The Rust side, as the page sees it. Every command is in
// src-tauri/src/commands.rs; the types mirror the structs there.

import { invoke } from '@tauri-apps/api/core'

export interface Session {
  /** The signed-in addresses, in the order they were added. */
  accounts: string[]
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
  /** The address of the account the thread is in. */
  account: string
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
}

/** The body of the message a card shows. Mirrors `MessageContent` in src-tauri/src/card.rs. */
export interface MessageContent {
  body: CardBody
  attachments: string[]
}

export interface Label {
  id: string
  name: string
}

/** An account whose part of the inbox could not be fetched. */
export interface AccountFailure {
  account: string
  kind: ErrorKind
  message: string
}

/** Mirrors `InboxPage` in src-tauri/src/accounts.rs. */
export interface InboxPage {
  cards: Card[]
  failures: AccountFailure[]
}

/** A custom view. Mirrors `View` in src-tauri/src/views.rs. */
export interface View {
  id: string
  name: string
  /** The accounts it shows; every account when empty. */
  accounts: string[]
  /** A Gmail search; nothing when empty. */
  query: string
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
  /** Without a client, the one the first account signed in with is used. */
  signIn: (clientId: string | null, clientSecret: string | null) =>
    invoke<string>('sign_in', { clientId, clientSecret }),
  cancelSignIn: () => invoke<void>('cancel_sign_in'),
  signOut: (account: string) => invoke<void>('sign_out', { account }),
  inbox: (accounts: string[], query: string) => invoke<InboxPage>('list_inbox', { accounts, query }),
  message: (account: string, messageId: string) => invoke<MessageContent>('get_message', { account, messageId }),
  labels: (account: string) => invoke<Label[]>('list_labels', { account }),
  createLabel: (account: string, name: string) => invoke<Label>('create_label', { account, name }),
  archive: (account: string, messageIds: string[]) => invoke<void>('archive', { account, messageIds }),
  applyLabel: (account: string, messageIds: string[], labelId: string) =>
    invoke<void>('apply_label', { account, messageIds, labelId }),
  setAside: (account: string, messageIds: string[]) => invoke<void>('set_aside', { account, messageIds }),
  setStarred: (account: string, messageId: string, messageIds: string[], starred: boolean) =>
    invoke<void>('set_starred', { account, messageId, messageIds, starred }),
  views: () => invoke<View[]>('list_views'),
  saveViews: (views: View[]) => invoke<void>('save_views', { views }),
  openLink: (url: string) => invoke<void>('open_link', { url }),
}
