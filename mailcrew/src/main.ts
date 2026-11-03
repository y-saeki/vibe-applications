import './style.css'

import { backend, errorMessage, isCommandError, type Session, type View } from './backend'
import { Inbox } from './inbox'
import { showNotice } from './notice'
import { Settings } from './settings'
import { filterFor, INBOX_VIEW, tabExists, ViewTabs } from './views'

const setup = document.querySelector<HTMLElement>('#setup')!
const setupForm = document.querySelector<HTMLFormElement>('#setup-form')!
const setupError = document.querySelector<HTMLElement>('#setup-error')!
const setupWaiting = document.querySelector<HTMLElement>('#setup-waiting')!
const signInButton = document.querySelector<HTMLButtonElement>('#sign-in')!
const clientId = document.querySelector<HTMLInputElement>('#client-id')!
const clientSecret = document.querySelector<HTMLInputElement>('#client-secret')!
const loading = document.querySelector<HTMLElement>('#loading')!

let session: Session | null = null
let views: View[] = []
let activeView = INBOX_VIEW

const accounts = () => session?.accounts ?? []

const inbox = new Inbox((message) => void signedOut(message))
const tabs = new ViewTabs((id) => {
  activeView = id
  showView()
})
const settings = new Settings({
  accounts,
  views: () => views,
  accountsChanged: async () => {
    if (await refreshSession()) settings.render()
  },
  viewsChanged: (saved) => {
    views = saved
    if (!tabExists(activeView, views)) activeView = INBOX_VIEW
    tabs.render(views, activeView)
  },
  close: () => {
    settings.hide()
    showInbox()
  },
})

function showSetup(message?: string): void {
  inbox.hide()
  settings.hide()
  loading.hidden = true
  setup.hidden = false
  setupWaiting.hidden = true
  signInButton.disabled = false
  setupError.textContent = message ?? ''
  setupError.hidden = !message
  clientId.focus()
}

function showInbox(): void {
  setup.hidden = true
  inbox.setAccounts(accounts())
  inbox.show()
  showView()
}

/** Lays out the tabs and fetches what the active one shows. */
function showView(): void {
  tabs.render(views, activeView)
  inbox.setFilter(filterFor(activeView, views, accounts()))
  void inbox.load()
}

/**
 * Asks who is signed in now. Resolves with false, having gone back to the
 * sign-in, when nobody is left.
 */
async function refreshSession(message?: string): Promise<boolean> {
  try {
    session = await backend.session()
  } catch (err) {
    showNotice(errorMessage(err))
    return true
  }
  if (accounts().length === 0) {
    showSetup(message)
    return false
  }
  inbox.setAccounts(accounts())
  return true
}

/** Google no longer accepts an account, which the Rust side has removed. */
async function signedOut(message: string): Promise<void> {
  if (await refreshSession(message)) showNotice(message)
}

setupForm.addEventListener('submit', (event) => {
  event.preventDefault()
  setupError.hidden = true
  signInButton.disabled = true
  setupWaiting.hidden = false
  backend
    .signIn(clientId.value, clientSecret.value)
    .then(async () => {
      clientSecret.value = ''
      if (await refreshSession()) showInbox()
    })
    .catch((err: unknown) => {
      // A cancelled sign-in is what the user asked for; nothing to report.
      showSetup(isCommandError(err) && err.kind === 'cancelled' ? undefined : errorMessage(err))
    })
})

document.querySelector('#cancel-sign-in')!.addEventListener('click', () => {
  void backend.cancelSignIn()
})

document.querySelector('#setup-help')!.addEventListener('click', (event) => {
  event.preventDefault()
  const href = (event.currentTarget as HTMLAnchorElement).href
  backend.openLink(href).catch((err: unknown) => showNotice(errorMessage(err)))
})

document.querySelector('#reload')!.addEventListener('click', () => void inbox.load())

document.querySelector('#open-settings')!.addEventListener('click', () => {
  inbox.hide()
  settings.open()
})

document.addEventListener('keydown', (event) => {
  const mod = session?.platform === 'macos' ? event.metaKey : event.ctrlKey
  // Reloading means the inbox, never the page itself.
  if (mod && event.key.toLowerCase() === 'r') {
    event.preventDefault()
    if (inbox.isShown) void inbox.load()
  }
})

async function main(): Promise<void> {
  try {
    session = await backend.session()
  } catch (err) {
    showSetup(errorMessage(err))
    return
  }
  try {
    views = await backend.views()
  } catch (err) {
    // The whole inbox does without them.
    showNotice(errorMessage(err))
  }
  if (accounts().length > 0) showInbox()
  else showSetup()
}

void main()
