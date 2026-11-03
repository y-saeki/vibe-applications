import './style.css'

import { backend, errorMessage, isCommandError, type Session } from './backend'
import { Inbox } from './inbox'
import { showNotice } from './notice'

const setup = document.querySelector<HTMLElement>('#setup')!
const setupForm = document.querySelector<HTMLFormElement>('#setup-form')!
const setupError = document.querySelector<HTMLElement>('#setup-error')!
const setupWaiting = document.querySelector<HTMLElement>('#setup-waiting')!
const signInButton = document.querySelector<HTMLButtonElement>('#sign-in')!
const clientId = document.querySelector<HTMLInputElement>('#client-id')!
const clientSecret = document.querySelector<HTMLInputElement>('#client-secret')!
const loading = document.querySelector<HTMLElement>('#loading')!

let session: Session | null = null

const inbox = new Inbox((message) => showSetup(message))

function showSetup(message?: string): void {
  inbox.hide()
  loading.hidden = true
  setup.hidden = false
  setupWaiting.hidden = true
  signInButton.disabled = false
  setupError.textContent = message ?? ''
  setupError.hidden = !message
  clientId.focus()
}

function showInbox(email: string): void {
  setup.hidden = true
  inbox.show(email)
  void inbox.load()
}

setupForm.addEventListener('submit', (event) => {
  event.preventDefault()
  setupError.hidden = true
  signInButton.disabled = true
  setupWaiting.hidden = false
  backend
    .signIn(clientId.value, clientSecret.value)
    .then((email) => {
      clientSecret.value = ''
      showInbox(email)
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

document.querySelector('#sign-out')!.addEventListener('click', () => {
  backend
    .signOut()
    .then(() => showSetup())
    .catch((err: unknown) => showNotice(errorMessage(err)))
})

document.addEventListener('keydown', (event) => {
  const mod = session?.platform === 'macos' ? event.metaKey : event.ctrlKey
  // Reloading means the inbox, never the page itself.
  if (mod && event.key.toLowerCase() === 'r') {
    event.preventDefault()
    if (!document.querySelector<HTMLElement>('#inbox')!.hidden) void inbox.load()
  }
})

async function main(): Promise<void> {
  try {
    session = await backend.session()
  } catch (err) {
    showSetup(errorMessage(err))
    return
  }
  if (session.email) showInbox(session.email)
  else showSetup()
}

void main()
