// The settings screen: the accounts Mail Crew reads, the custom views, and the
// senders whose images are shown without asking.

import { backend, errorMessage, isCommandError, type View } from './backend'
import type { ImageSenders } from './image-senders'
import { describeView } from './views'

export interface SettingsHost {
  accounts(): string[]
  views(): View[]
  imageSenders: ImageSenders
  /** Asks who is signed in again, after an account came or went. */
  accountsChanged(): Promise<void>
  /** The views, once they are saved. */
  viewsChanged(views: View[]): void
  close(): void
}

function newViewId(): string {
  return `v${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`
}

export class Settings {
  private readonly root = document.querySelector<HTMLElement>('#settings')!
  private readonly accountList = document.querySelector<HTMLUListElement>('#account-list')!
  private readonly accountForm = document.querySelector<HTMLFormElement>('#account-form')!
  private readonly accountError = document.querySelector<HTMLElement>('#account-error')!
  private readonly accountWaiting = document.querySelector<HTMLElement>('#account-waiting')!
  private readonly addAccount = document.querySelector<HTMLButtonElement>('#add-account')!
  private readonly clientId = document.querySelector<HTMLInputElement>('#account-client-id')!
  private readonly clientSecret = document.querySelector<HTMLInputElement>('#account-client-secret')!
  private readonly otherClient = document.querySelector<HTMLDetailsElement>('#other-client')!
  private readonly viewList = document.querySelector<HTMLUListElement>('#view-list')!
  private readonly viewForm = document.querySelector<HTMLFormElement>('#view-form')!
  private readonly viewName = document.querySelector<HTMLInputElement>('#view-name')!
  private readonly viewAccounts = document.querySelector<HTMLElement>('#view-accounts')!
  private readonly viewQuery = document.querySelector<HTMLInputElement>('#view-query')!
  private readonly viewError = document.querySelector<HTMLElement>('#view-error')!
  private readonly addView = document.querySelector<HTMLButtonElement>('#add-view')!
  private readonly senderList = document.querySelector<HTMLUListElement>('#image-sender-list')!
  private readonly senderError = document.querySelector<HTMLElement>('#image-sender-error')!
  /** The view being edited; null while a new one is being made. */
  private editing: string | null = null

  constructor(private readonly host: SettingsHost) {
    document.querySelector('#close-settings')!.addEventListener('click', () => host.close())

    this.accountForm.addEventListener('submit', (event) => {
      event.preventDefault()
      void this.signIn()
    })
    document.querySelector('#cancel-add-account')!.addEventListener('click', () => {
      void backend.cancelSignIn()
    })
    this.accountList.addEventListener('click', (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>('button[data-remove-account]')
      if (button) void this.remove(button)
    })

    this.addView.addEventListener('click', () => this.editView(null))
    this.viewList.addEventListener('click', (event) => {
      const edit = (event.target as Element).closest<HTMLButtonElement>('button[data-edit-view]')
      if (edit) this.editView(edit.dataset.editView ?? null)
      const remove = (event.target as Element).closest<HTMLButtonElement>('button[data-delete-view]')
      if (remove) void this.saveViews(this.host.views().filter((view) => view.id !== remove.dataset.deleteView))
    })
    this.viewForm.addEventListener('submit', (event) => {
      event.preventDefault()
      void this.submitView()
    })
    document.querySelector('#cancel-view')!.addEventListener('click', () => this.closeViewForm())

    this.senderList.addEventListener('click', (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>('button[data-remove-sender]')
      if (button) void this.removeSender(button)
    })
  }

  get isShown(): boolean {
    return !this.root.hidden
  }

  open(): void {
    this.root.hidden = false
    this.accountError.hidden = true
    this.senderError.hidden = true
    this.closeViewForm()
    this.render()
  }

  hide(): void {
    this.root.hidden = true
  }

  /** Lays out every list again from what the host holds now. */
  render(): void {
    const accounts = this.host.accounts()
    this.accountList.replaceChildren(
      ...accounts.map((account) => {
        const item = document.createElement('li')
        item.className = 'settings-row'
        const name = document.createElement('span')
        name.className = 'settings-row-name'
        name.textContent = account
        const remove = document.createElement('button')
        remove.type = 'button'
        remove.className = 'button button-quiet'
        remove.dataset.removeAccount = account
        remove.textContent = '削除'
        item.append(name, remove)
        return item
      }),
    )

    const views = this.host.views()
    this.viewList.replaceChildren(
      ...views.map((view) => {
        const item = document.createElement('li')
        item.className = 'settings-row'
        item.dataset.view = view.id
        const text = document.createElement('div')
        text.className = 'settings-row-text'
        const name = document.createElement('span')
        name.className = 'settings-row-name'
        name.textContent = view.name
        const summary = document.createElement('span')
        summary.className = 'settings-row-summary'
        summary.textContent = describeView(view)
        text.append(name, summary)
        const edit = document.createElement('button')
        edit.type = 'button'
        edit.className = 'button button-quiet'
        edit.dataset.editView = view.id
        edit.textContent = '編集'
        const remove = document.createElement('button')
        remove.type = 'button'
        remove.className = 'button button-quiet'
        remove.dataset.deleteView = view.id
        remove.textContent = '削除'
        item.append(text, edit, remove)
        return item
      }),
    )
    if (views.length === 0) {
      const item = document.createElement('li')
      item.className = 'settings-row settings-row-none'
      item.textContent = 'ビューはまだありません'
      this.viewList.append(item)
    }

    const senders = this.host.imageSenders.list()
    this.senderList.replaceChildren(
      ...senders.map((sender) => {
        const item = document.createElement('li')
        item.className = 'settings-row'
        const name = document.createElement('span')
        name.className = 'settings-row-name'
        name.textContent = sender
        const remove = document.createElement('button')
        remove.type = 'button'
        remove.className = 'button button-quiet'
        remove.dataset.removeSender = sender
        remove.textContent = '削除'
        item.append(name, remove)
        return item
      }),
    )
    if (senders.length === 0) {
      const item = document.createElement('li')
      item.className = 'settings-row settings-row-none'
      item.textContent = '画像を常に表示する差出人はいません'
      this.senderList.append(item)
    }
  }

  private async removeSender(button: HTMLButtonElement): Promise<void> {
    this.senderError.hidden = true
    button.disabled = true
    try {
      await this.host.imageSenders.remove(button.dataset.removeSender!)
    } catch (err) {
      button.disabled = false
      this.senderError.textContent = errorMessage(err)
      this.senderError.hidden = false
      return
    }
    this.render()
  }

  private async signIn(): Promise<void> {
    this.accountError.hidden = true
    this.addAccount.disabled = true
    this.accountWaiting.hidden = false
    try {
      // Left empty, the client the first account signed in with is used.
      await backend.signIn(this.clientId.value.trim() || null, this.clientSecret.value.trim() || null)
      this.clientId.value = ''
      this.clientSecret.value = ''
      this.otherClient.open = false
      await this.host.accountsChanged()
    } catch (err) {
      // A cancelled sign-in is what the user asked for; nothing to report.
      if (!(isCommandError(err) && err.kind === 'cancelled')) this.showAccountError(errorMessage(err))
    } finally {
      this.addAccount.disabled = false
      this.accountWaiting.hidden = true
    }
  }

  private async remove(button: HTMLButtonElement): Promise<void> {
    this.accountError.hidden = true
    button.disabled = true
    try {
      await backend.signOut(button.dataset.removeAccount!)
      await this.host.accountsChanged()
    } catch (err) {
      button.disabled = false
      this.showAccountError(errorMessage(err))
    }
  }

  private showAccountError(message: string): void {
    this.accountError.textContent = message
    this.accountError.hidden = false
  }

  /** Opens the view form, filled in from the view with `id`, or empty for a new one. */
  private editView(id: string | null): void {
    const view = this.host.views().find((known) => known.id === id)
    this.editing = view?.id ?? null
    this.viewName.value = view?.name ?? ''
    this.viewQuery.value = view?.query ?? ''
    this.viewAccounts.replaceChildren(
      ...this.host.accounts().map((account) => {
        const label = document.createElement('label')
        label.className = 'check'
        const box = document.createElement('input')
        box.type = 'checkbox'
        box.value = account
        box.checked = view?.accounts.includes(account) ?? false
        const text = document.createElement('span')
        text.textContent = account
        label.append(box, text)
        return label
      }),
    )
    this.viewError.hidden = true
    this.viewForm.hidden = false
    this.addView.hidden = true
    this.viewName.focus()
  }

  private closeViewForm(): void {
    this.viewForm.hidden = true
    this.viewError.hidden = true
    this.addView.hidden = false
    this.editing = null
  }

  private async submitView(): Promise<void> {
    const name = this.viewName.value.trim()
    if (!name) {
      this.viewError.textContent = 'ビューの名前を入力してください'
      this.viewError.hidden = false
      return
    }
    const accounts = [...this.viewAccounts.querySelectorAll<HTMLInputElement>('input:checked')].map((box) => box.value)
    const view: View = { id: this.editing ?? newViewId(), name, accounts, query: this.viewQuery.value.trim() }
    const views = this.host.views()
    const next = this.editing ? views.map((known) => (known.id === view.id ? view : known)) : [...views, view]
    if (await this.saveViews(next)) this.closeViewForm()
  }

  private async saveViews(views: View[]): Promise<boolean> {
    try {
      await backend.saveViews(views)
    } catch (err) {
      this.viewError.textContent = errorMessage(err)
      this.viewError.hidden = false
      return false
    }
    this.viewError.hidden = true
    this.host.viewsChanged(views)
    this.render()
    return true
  }
}
