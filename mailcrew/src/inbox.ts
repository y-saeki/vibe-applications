// The inbox: one card per thread, newest at the left, each with its toolbar.
// The threads of every account the current view covers share the strip, and
// each card's actions go to the account its thread is in. Every action takes
// the card off the strip at once and puts it back if the Rust side reports a
// failure.

import { backend, errorMessage, isCommandError, type Card, type InboxPage, type Label } from './backend'
import { formatDate } from './format'
import { ICONS } from './icons'
import { mountBody, type MountedBody } from './message-frame'
import { showNotice } from './notice'
import { OverlayScrollbar } from './overlay-scrollbar'
import { TagPicker, type TagChoice } from './tag-picker'

/** How far beyond the visible strip a card's body is already rendered. */
const PRELOAD_MARGIN = '0px 1200px'

type Action = 'done' | 'tag' | 'later' | 'star'

const TOOLS: { action: Action; label: string }[] = [
  { action: 'done', label: '完了' },
  { action: 'tag', label: 'タグ' },
  { action: 'later', label: 'あとで読む' },
  { action: 'star', label: 'スター' },
]

/** What the strip shows: the accounts (all of them when empty) and a Gmail search. */
export interface InboxFilter {
  accounts: string[]
  query: string
  /** A custom view rather than the whole inbox, which changes what "empty" says. */
  custom: boolean
}

/** A view whose accounts have all been removed shows nothing at all. */
export type Filter = InboxFilter | null

const ALL: InboxFilter = { accounts: [], query: '', custom: false }

/** Thread ids are only unique within one account. */
function keyOf(card: Card): string {
  return `${card.account}\n${card.id}`
}

interface Mounted {
  card: Card
  element: HTMLElement
  body: MountedBody | null
}

export class Inbox {
  private readonly root = document.querySelector<HTMLElement>('#inbox')!
  private readonly strip = document.querySelector<HTMLElement>('#strip')!
  private readonly empty = document.querySelector<HTMLElement>('#empty')!
  private readonly count = document.querySelector<HTMLElement>('#inbox-count')!
  private readonly loading = document.querySelector<HTMLElement>('#loading')!
  private readonly emptyTitle = document.querySelector<HTMLElement>('#empty-title')!
  private readonly emptyLead = document.querySelector<HTMLElement>('#empty-lead')!
  private readonly picker = new TagPicker()
  private readonly mounted = new Map<string, Mounted>()
  /** Each account's labels, as last fetched. */
  private readonly labels = new Map<string, Label[]>()
  private filter: Filter = ALL
  private loadingNow = false
  /** Set when a load is asked for while one is running, so the newer filter wins. */
  private loadAgain = false
  private readonly observer = new IntersectionObserver((entries) => this.reveal(entries), {
    root: this.strip,
    rootMargin: PRELOAD_MARGIN,
  })

  constructor(private readonly onSignedOut: (message: string) => void) {
    this.strip.addEventListener('click', (event) => this.onClick(event))
    // A card is narrower than its usual width only when the window is, so a
    // message is laid out again when the window changes size.
    window.addEventListener('resize', () => {
      for (const mounted of this.mounted.values()) mounted.body?.refit()
    })
    // A mouse wheel scrolls the strip sideways; inside a message it still
    // scrolls the message, since the frame keeps its own events.
    this.strip.addEventListener(
      'wheel',
      (event) => {
        if (event.ctrlKey || Math.abs(event.deltaX) >= Math.abs(event.deltaY)) return
        const body = (event.target as Element).closest('.card-body')
        if (body && body.scrollHeight > body.clientHeight) return
        this.strip.scrollLeft += event.deltaY
        event.preventDefault()
      },
      { passive: false },
    )
  }

  show(): void {
    this.root.hidden = false
  }

  hide(): void {
    this.picker.close()
    this.root.hidden = true
    this.clear()
  }

  get isShown(): boolean {
    return !this.root.hidden
  }

  /**
   * The signed-in accounts. A card says which account it is in only when
   * there is more than one, and the cards of an account that has gone leave
   * the strip.
   */
  setAccounts(accounts: string[]): void {
    this.root.classList.toggle('inbox-multi', accounts.length > 1)
    for (const account of this.labels.keys()) if (!accounts.includes(account)) this.labels.delete(account)
    for (const [key, mounted] of this.mounted) {
      if (accounts.includes(mounted.card.account)) continue
      this.mounted.delete(key)
      mounted.element.remove()
    }
    this.updateCount()
  }

  /** What the next load fetches. */
  setFilter(filter: Filter): void {
    this.filter = filter
  }

  /** Fetches the newest threads and lays them out from the start of the strip. */
  async load(): Promise<void> {
    if (this.loadingNow) {
      this.loadAgain = true
      return
    }
    this.loadingNow = true
    this.picker.close()
    this.loading.hidden = false
    try {
      do {
        this.loadAgain = false
        const filter = this.filter
        const page: InboxPage = filter ? await backend.inbox(filter.accounts, filter.query) : { cards: [], failures: [] }
        if (this.loadAgain) continue
        this.render(page.cards)
        this.refreshLabels()
        this.reportFailures(page)
      } while (this.loadAgain)
    } catch (err) {
      this.fail(err)
    } finally {
      this.loading.hidden = true
      this.loadingNow = false
    }
  }

  /** Says which accounts did not answer; the others' cards are on the strip all the same. */
  private reportFailures(page: InboxPage): void {
    if (page.failures.length === 0) return
    const message = page.failures.map((failure) => `${failure.account}: ${failure.message}`).join('\n')
    // An account Google no longer accepts has been removed on the Rust side.
    if (page.failures.some((failure) => failure.kind === 'signedOut')) this.onSignedOut(message)
    else showNotice(message)
  }

  private clear(): void {
    this.observer.disconnect()
    this.mounted.clear()
    this.strip.replaceChildren()
  }

  private render(cards: Card[]): void {
    this.clear()
    const custom = this.filter?.custom ?? true
    this.emptyTitle.textContent = custom ? 'このビューに当てはまるメールはありません' : '受信トレイは空です'
    this.emptyLead.textContent = custom
      ? '条件に合うメールが届いたら、再読み込みで表示されます。'
      : '新しいメールが届いたら、再読み込みで表示されます。'
    for (const card of cards) {
      const element = this.buildCard(card)
      this.mounted.set(keyOf(card), { card, element, body: null })
      this.strip.append(element)
      this.observer.observe(element)
    }
    if (cards.length > 0) this.strip.append(this.buildEnd())
    this.strip.scrollLeft = 0
    this.updateCount()
  }

  private updateCount(): void {
    const count = this.mounted.size
    this.count.textContent = count > 0 ? `${count} 件` : ''
    this.empty.hidden = count > 0
    this.strip.hidden = count === 0
    const summary = this.strip.querySelector('.end-summary')
    if (summary) summary.textContent = `ここまでの ${count} 件を既読にしてアーカイブし、次の受信トレイを読み込みます。`
  }

  /** Fetches the labels of every account on the strip. */
  private refreshLabels(): void {
    const accounts = new Set([...this.mounted.values()].map((mounted) => mounted.card.account))
    for (const account of accounts) void this.fetchLabels(account)
  }

  private async fetchLabels(account: string): Promise<Label[]> {
    try {
      const labels = await backend.labels(account)
      this.labels.set(account, labels)
      this.picker.setLabels(account, labels)
      return labels
    } catch {
      // The picker asks again when it opens; nothing to say yet.
      return []
    }
  }

  private buildCard(card: Card): HTMLElement {
    const element = document.createElement('article')
    element.className = 'card'
    element.dataset.threadId = card.id
    element.dataset.account = card.account
    element.dataset.key = keyOf(card)
    if (card.unread) element.classList.add('card-unread')

    const head = document.createElement('header')
    head.className = 'card-head'
    const fromRow = document.createElement('div')
    fromRow.className = 'card-from-row'
    const from = document.createElement('span')
    from.className = 'card-from'
    from.textContent = card.from.name || card.from.email
    from.title = card.from.email
    const date = document.createElement('time')
    date.className = 'card-date'
    date.dateTime = new Date(card.date).toISOString()
    date.textContent = formatDate(card.date)
    fromRow.append(from, date)
    const subject = document.createElement('h2')
    subject.className = 'card-subject'
    subject.textContent = card.subject || '(件名なし)'
    head.append(fromRow, subject)

    const meta = document.createElement('div')
    meta.className = 'card-meta'
    // Shown only while more than one account is signed in.
    const account = document.createElement('span')
    account.className = 'card-badge card-account'
    account.textContent = card.account
    account.title = card.account
    meta.append(account)
    if (card.messageCount > 1) {
      const count = document.createElement('span')
      count.className = 'card-badge card-message-count'
      count.textContent = `${card.messageCount} 通`
      meta.append(count)
    }
    if (card.attachments.length > 0) {
      const attachments = document.createElement('span')
      attachments.className = 'card-badge card-attachments'
      attachments.innerHTML = ICONS.attachment
      attachments.append(` ${card.attachments.length}`)
      attachments.title = card.attachments.join('\n')
      meta.append(attachments)
    }
    // Always there, empty or not, so that every card's head is the same height.
    head.append(meta)

    const images = document.createElement('div')
    images.className = 'card-images'
    images.hidden = true
    const imagesText = document.createElement('span')
    imagesText.textContent = '外部の画像を読み込んでいません'
    const imagesButton = document.createElement('button')
    imagesButton.type = 'button'
    imagesButton.className = 'link-button'
    imagesButton.dataset.action = 'images'
    imagesButton.textContent = '表示する'
    images.append(imagesText, imagesButton)

    const bodyWrap = document.createElement('div')
    bodyWrap.className = 'card-body-wrap'
    const body = document.createElement('div')
    body.className = 'card-body'
    const snippet = document.createElement('p')
    snippet.className = 'card-snippet'
    snippet.textContent = card.snippet
    body.append(snippet)
    bodyWrap.append(body)

    const toolbar = document.createElement('footer')
    toolbar.className = 'card-toolbar'
    for (const tool of TOOLS) {
      const button = document.createElement('button')
      button.type = 'button'
      button.className = `tool tool-${tool.action}`
      button.dataset.action = tool.action
      button.title = tool.label
      button.innerHTML = ICONS[tool.action]
      const text = document.createElement('span')
      text.textContent = tool.label
      button.append(text)
      if (tool.action === 'star') button.setAttribute('aria-pressed', String(card.starred))
      toolbar.append(button)
    }

    element.append(head, images, bodyWrap, toolbar)
    return element
  }

  private buildEnd(): HTMLElement {
    const element = document.createElement('div')
    element.className = 'card card-end'
    const summary = document.createElement('p')
    summary.className = 'end-summary'
    const button = document.createElement('button')
    button.type = 'button'
    button.id = 'done-all'
    button.className = 'button button-primary'
    button.dataset.action = 'done-all'
    button.textContent = 'まとめて完了'
    element.append(summary, button)
    return element
  }

  /** Renders the bodies of cards as they come near the visible part of the strip. */
  private reveal(entries: IntersectionObserverEntry[]): void {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue
      const element = entry.target as HTMLElement
      this.observer.unobserve(element)
      const mounted = this.mounted.get(element.dataset.key ?? '')
      if (!mounted || mounted.body) continue
      const container = element.querySelector<HTMLElement>('.card-body')!
      mounted.body = mountBody(container, mounted.card.body, (url) => this.openLink(url))
      const scrollbar = new OverlayScrollbar(container, container.parentElement!)
      for (const child of container.children) scrollbar.watch(child)
      const body = mounted.card.body
      if (body.kind === 'html' && body.remoteImages) {
        element.querySelector<HTMLElement>('.card-images')!.hidden = false
      }
    }
  }

  private openLink(url: string): void {
    backend.openLink(url).catch((err: unknown) => showNotice(errorMessage(err)))
  }

  private onClick(event: MouseEvent): void {
    const button = (event.target as Element).closest<HTMLButtonElement>('button[data-action]')
    if (!button || button.disabled) return
    if (button.dataset.action === 'done-all') {
      void this.doneAll(button)
      return
    }
    const element = button.closest<HTMLElement>('.card')
    const mounted = this.mounted.get(element?.dataset.key ?? '')
    if (!mounted) return
    const { card } = mounted
    switch (button.dataset.action) {
      case 'done':
        void this.takeOff(mounted, () => backend.archive(card.account, card.messageIds))
        break
      case 'later':
        void this.takeOff(mounted, () => backend.setAside(card.account, card.messageIds))
        break
      case 'tag':
        void this.pickTag(mounted, button)
        break
      case 'star':
        void this.toggleStar(mounted, button)
        break
      case 'images':
        mounted.body?.allowRemoteImages()
        button.closest<HTMLElement>('.card-images')!.hidden = true
        break
    }
  }

  /** Takes the card off the strip, and puts it back where it was if `action` fails. */
  private async takeOff(mounted: Mounted, action: () => Promise<void>): Promise<void> {
    const { element, card } = mounted
    const next = element.nextSibling
    this.mounted.delete(keyOf(card))
    element.remove()
    this.updateCount()
    try {
      await action()
    } catch (err) {
      this.strip.insertBefore(element, next?.isConnected ? next : this.strip.querySelector('.card-end'))
      this.mounted.set(keyOf(card), mounted)
      this.updateCount()
      this.fail(err)
      return
    }
    // The strip shows a page of the inbox at a time; once it has been worked
    // through, the next page is fetched.
    if (this.mounted.size === 0) await this.load()
  }

  private async pickTag(mounted: Mounted, button: HTMLButtonElement): Promise<void> {
    const { account } = mounted.card
    if (!this.labels.get(account)?.length) void this.fetchLabels(account)
    const choice: TagChoice | null = await this.picker.open(button, account)
    if (!choice) return
    await this.takeOff(mounted, async () => {
      const label = choice.kind === 'existing' ? choice.label : await backend.createLabel(account, choice.name)
      if (choice.kind === 'new') {
        const known = (this.labels.get(account) ?? []).filter((other) => other.id !== label.id)
        const labels = [...known, label].sort((a, b) => a.name.localeCompare(b.name))
        this.labels.set(account, labels)
        this.picker.setLabels(account, labels)
      }
      await backend.applyLabel(account, mounted.card.messageIds, label.id)
    })
  }

  private async toggleStar(mounted: Mounted, button: HTMLButtonElement): Promise<void> {
    const { card } = mounted
    const starred = !card.starred
    card.starred = starred
    button.setAttribute('aria-pressed', String(starred))
    button.disabled = true
    try {
      await backend.setStarred(card.account, card.messageId, card.messageIds, starred)
    } catch (err) {
      card.starred = !starred
      button.setAttribute('aria-pressed', String(!starred))
      this.fail(err)
    } finally {
      button.disabled = false
    }
  }

  /** Archives every card on the strip, one request per account, then fetches the next page. */
  private async doneAll(button: HTMLButtonElement): Promise<void> {
    const byAccount = new Map<string, string[]>()
    for (const { card } of this.mounted.values()) {
      byAccount.set(card.account, [...(byAccount.get(card.account) ?? []), ...card.messageIds])
    }
    if (byAccount.size === 0) return
    button.disabled = true
    const results = await Promise.allSettled(
      [...byAccount].map(([account, ids]) => backend.archive(account, ids)),
    )
    const failure = results.find((result): result is PromiseRejectedResult => result.status === 'rejected')
    if (failure && results.every((result) => result.status === 'rejected')) {
      button.disabled = false
      this.fail(failure.reason)
      return
    }
    // Some accounts' threads are archived and some are not: the next page
    // shows which are still there.
    await this.load()
    if (failure) this.fail(failure.reason)
  }

  private fail(err: unknown): void {
    if (isCommandError(err) && err.kind === 'signedOut') {
      this.onSignedOut(err.message)
      return
    }
    showNotice(errorMessage(err))
  }
}
