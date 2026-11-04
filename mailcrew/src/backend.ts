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

/** One method's verdict in Gmail's checks. Mirrors `Check` in src-tauri/src/authres.rs. */
export interface Check {
  /** `pass`, `fail`, `softfail`, `neutral`, `none`, `temperror`, `permerror` or `policy`. */
  result: string
  /** The domain the result is about; empty when Gmail does not say. */
  domain: string
}

/** What Gmail found when it checked the sender. Mirrors `Authentication` in src-tauri/src/authres.rs. */
export interface Authentication {
  spf: Check | null
  dkim: Check | null
  dmarc: Check | null
}

/** A file attached to a message. Mirrors `Attachment` in src-tauri/src/card.rs. */
export interface Attachment {
  /** The part's id within the message; the open and save commands take it. */
  id: string
  name: string
  mimeType: string
  /** Bytes. */
  size: number
  /** Whether Mail Crew opens it; otherwise it can only be saved. */
  openable: boolean
}

/** The body of the message a card shows. Mirrors `MessageContent` in src-tauri/src/card.rs. */
export interface MessageContent {
  body: CardBody
  attachments: Attachment[]
  authentication: Authentication
}

/** Mirrors `BimiResult` in src-tauri/src/bimi.rs. */
export type BimiResult = 'pass' | 'none' | 'declined' | 'fail' | 'skipped' | 'temperror'

/** Mirrors `MarkResult` in src-tauri/src/bimi.rs. */
export type MarkResult = 'none' | 'vmc' | 'cmc' | 'fail' | 'temperror'

/** The sender's BIMI record and mark certificate. Mirrors `Brand` in src-tauri/src/bimi.rs. */
export interface Brand {
  bimi: BimiResult
  /** The domain whose BIMI record was used; empty when none was. */
  domain: string
  mark: MarkResult
  /** The certificate's logo as a `data:image/svg+xml` URL. */
  logo: string | null
}

/** Mirrors `Preferences` in src-tauri/src/preferences.rs. */
export interface Preferences {
  /** Look up senders' BIMI records and mark certificates. */
  brandLogos: boolean
}

export interface Label {
  id: string
  name: string
}

/** Which pile the strip shows. Mirrors `Mailbox` in src-tauri/src/gmail.rs. */
export type Mailbox = 'inbox' | 'setAside'

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
  inbox: (accounts: string[], mailbox: Mailbox, query: string) =>
    invoke<InboxPage>('list_inbox', { accounts, mailbox, query }),
  message: (account: string, messageId: string) => invoke<MessageContent>('get_message', { account, messageId }),
  /** Null when the user turned the lookups off. */
  brand: (account: string, messageId: string) => invoke<Brand | null>('get_brand', { account, messageId }),
  /** Opens an attachment in the app the OS picks; only kinds marked `openable`. */
  openAttachment: (account: string, messageId: string, attachmentId: string) =>
    invoke<void>('open_attachment', { account, messageId, attachmentId }),
  /** Saves an attachment to the downloads folder; resolves with the name it was saved under. */
  saveAttachment: (account: string, messageId: string, attachmentId: string) =>
    invoke<string>('save_attachment', { account, messageId, attachmentId }),
  preferences: () => invoke<Preferences>('get_preferences'),
  savePreferences: (preferences: Preferences) => invoke<void>('save_preferences', { preferences }),
  labels: (account: string) => invoke<Label[]>('list_labels', { account }),
  createLabel: (account: string, name: string) => invoke<Label>('create_label', { account, name }),
  /** 完了: read, and out of `mailbox`. */
  archive: (account: string, messageIds: string[], mailbox: Mailbox) =>
    invoke<void>('archive', { account, messageIds, mailbox }),
  /** タグ: tagged, and out of `mailbox`. */
  applyLabel: (account: string, messageIds: string[], labelId: string, mailbox: Mailbox) =>
    invoke<void>('apply_label', { account, messageIds, labelId, mailbox }),
  setAside: (account: string, messageIds: string[]) => invoke<void>('set_aside', { account, messageIds }),
  setStarred: (account: string, messageId: string, messageIds: string[], starred: boolean) =>
    invoke<void>('set_starred', { account, messageId, messageIds, starred }),
  views: () => invoke<View[]>('list_views'),
  saveViews: (views: View[]) => invoke<void>('save_views', { views }),
  /** The senders whose images are shown without asking. */
  imageSenders: () => invoke<string[]>('list_image_senders'),
  /** Resolves with the senders as saved: in lower case, each once. */
  saveImageSenders: (senders: string[]) => invoke<string[]>('save_image_senders', { senders }),
  openLink: (url: string) => invoke<void>('open_link', { url }),
}
