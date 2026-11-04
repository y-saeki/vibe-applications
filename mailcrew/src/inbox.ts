// The inbox: one card per thread, newest at the left, each with its toolbar.
// The threads of every account the current view covers share the strip, and
// each card's actions go to the account its thread is in. Every action takes
// the card off the strip at once and puts it back if the Rust side reports a
// failure.

import {
  backend,
  errorMessage,
  isCommandError,
  type Attachment,
  type Authentication,
  type Brand,
  type Card,
  type InboxPage,
  type Label,
  type Mailbox,
  type MessageContent,
} from './backend'
import { AttachmentMenu, type AttachmentAction } from './attachment-menu'
import { formatDate } from './format'
import { ICONS } from './icons'
import type { ImageSenders } from './image-senders'
import { mountBody, type MountedBody } from './message-frame'
import { showNotice } from './notice'
import { OverlayScrollbar } from './overlay-scrollbar'
import { PullToReload } from './pull-reload'
import type { PreferenceStore } from './preferences'
import { TagPicker, type TagChoice } from './tag-picker'
import { renderTrust } from './trust'

/** How far beyond the visible strip a card's body is already rendered. */
const PRELOAD_MARGIN = '0px 1200px'

type Action = 'done' | 'tag' | 'later' | 'star'

const TOOLS: { action: Action; label: string }[] = [
  { action: 'done', label: '完了' },
  { action: 'tag', label: 'タグ' },
  { action: 'later', label: 'あとで読む' },
  { action: 'star', label: 'スター' },
]

/**
 * What the strip shows: a pile (the inbox, or what あとで読む set aside), the
 * accounts (all of them when empty) and a Gmail search.
 */
export interface InboxFilter {
  mailbox: Mailbox
  accounts: string[]
  query: string
  /** A custom view rather than a whole pile, which changes what "empty" says. */
  custom: boolean
}

/** A view whose accounts have all been removed shows nothing at all. */
export type Filter = InboxFilter | null

const INBOX: InboxFilter = { mailbox: 'inbox', accounts: [], query: '', custom: false }

/** Thread ids are only unique within one account. */
function keyOf(card: Card): string {
  return `${card.account}\n${card.id}`
}

interface Mounted {
  card: Card
  element: HTMLElement
  body: MountedBody | null
  /** Set while the message's body is being fetched. */
  fetching: boolean
  /** The message's attachments, once its body has been fetched. */
  attachments: Attachment[]
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
  private readonly attachmentMenu = new AttachmentMenu((err) => this.fail(err))
  private readonly mounted = new Map<string, Mounted>()
  /** Each account's labels, as last fetched. */
  private readonly labels = new Map<string, Label[]>()
  private filter: Filter = INBOX
  /** The pile the cards on the strip came from; every action takes a card out of it. */
  private mailbox: Mailbox = 'inbox'
  private loadingNow = false
  /** Set when a load is asked for while one is running, so the newer filter wins. */
  private loadAgain = false
  private readonly pull = new PullToReload(this.strip, () => this.load())
  private readonly observer = new IntersectionObserver((entries) => this.reveal(entries), {
    root: this.strip,
    rootMargin: PRELOAD_MARGIN,
  })

  constructor(
    private readonly onSignedOut: (message: string) => void,
    private readonly imageSenders: ImageSenders,
    private readonly preferences: PreferenceStore,
  ) {
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
        // A wheel only scrolls back to the first card; pulling past it is a swipe.
        this.strip.scrollLeft = Math.max(this.pull.rest, this.strip.scrollLeft + event.deltaY)
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
    this.attachmentMenu.close()
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
    this.attachmentMenu.close()
    this.loading.hidden = false
    try {
      do {
        this.loadAgain = false
        const filter = this.filter
        const page: InboxPage = filter
          ? await backend.inbox(filter.accounts, filter.mailbox, filter.query)
          : { cards: [], failures: [] }
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
    this.strip.replaceChildren(this.pull.zone)
  }

  private render(cards: Card[]): void {
    this.clear()
    this.mailbox = this.filter?.mailbox ?? 'inbox'
    const custom = this.filter?.custom ?? true
    if (custom) {
      this.emptyTitle.textContent = 'このビューに当てはまるメールはありません'
      this.emptyLead.textContent = '条件に合うメールが届いたら、再読み込みで表示されます。'
    } else if (this.mailbox === 'setAside') {
      this.emptyTitle.textContent = 'あとで読むメールはありません'
      this.emptyLead.textContent = '受信トレイで「あとで読む」を押したメールが、ここに並びます。'
    } else {
      this.emptyTitle.textContent = '受信トレイは空です'
      this.emptyLead.textContent = '新しいメールが届いたら、再読み込みで表示されます。'
    }
    for (const card of cards) {
      const element = this.buildCard(card)
      this.mounted.set(keyOf(card), { card, element, body: null, fetching: false, attachments: [] })
      this.strip.append(element)
      this.observer.observe(element)
    }
    if (cards.length > 0) this.strip.append(this.buildEnd())
    this.updateCount()
    this.pull.reset()
  }

  private updateCount(): void {
    const count = this.mounted.size
    this.count.textContent = count > 0 ? `${count} 件` : ''
    this.empty.hidden = count > 0
    this.strip.hidden = count === 0
    const summary = this.strip.querySelector('.end-summary')
    if (summary) {
      summary.textContent =
        this.mailbox === 'setAside'
          ? `ここまでの ${count} 件を既読にして「あとで読む」から外し、続きを読み込みます。`
          : `ここまでの ${count} 件を既読にしてアーカイブし、次の受信トレイを読み込みます。`
    }
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
    // The sender's BIMI logo, once its certificate has been checked.
    const logo = document.createElement('img')
    logo.className = 'card-logo'
    logo.alt = ''
    logo.hidden = true
    const from = document.createElement('span')
    from.className = 'card-from'
    from.textContent = card.from.name || card.from.email
    from.title = card.from.email
    const date = document.createElement('time')
    date.className = 'card-date'
    date.dateTime = new Date(card.date).toISOString()
    date.textContent = formatDate(card.date)
    const subject = document.createElement('h2')
    subject.className = 'card-subject'
    subject.textContent = card.subject || '(件名なし)'
    // Filled in with the body; the row keeps its height until then so that
    // every head is as tall.
    const trust = document.createElement('div')
    trust.className = 'card-trust'
    trust.setAttribute('role', 'group')
    trust.setAttribute('aria-label', '送信元の検証')
    head.append(fromRow, subject, trust)

    // The badges sit on the sender's line, between the name and the date.
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
    fromRow.append(logo, from, meta, date)

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
    const imagesActions = document.createElement('span')
    imagesActions.className = 'card-images-actions'
    imagesActions.append(imagesButton)
    if (card.from.email) {
      const always = document.createElement('button')
      always.type = 'button'
      always.className = 'link-button'
      always.dataset.action = 'images-always'
      always.textContent = 'この差出人は常に表示'
      always.title = `${card.from.email} からのメールでは、外部の画像を常に読み込みます`
      imagesActions.append(always)
    }
    images.append(imagesText, imagesActions)

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
      // A thread already set aside cannot be set aside again.
      if (tool.action === 'later' && this.mailbox === 'setAside') continue
      const button = document.createElement('button')
      button.type = 'button'
      button.className = `tool tool-${tool.action}`
      button.dataset.action = tool.action
      button.title = tool.label
      button.innerHTML = ICONS[tool.action]
      const text = document.createElement('span')
      text.className = 'tool-label'
      text.textContent = tool.label
      button.append(text)
      if (tool.action === 'star') button.setAttribute('aria-pressed', String(card.starred))
      toolbar.append(button)
    }
    // Last among the icons, so the others keep their places on every card.
    // The attachments are known only once the body has been fetched.
    const attachments = document.createElement('button')
    attachments.type = 'button'
    attachments.className = 'tool tool-attachments'
    attachments.dataset.action = 'attachments'
    attachments.setAttribute('aria-haspopup', 'dialog')
    attachments.setAttribute('aria-expanded', 'false')
    attachments.hidden = true
    toolbar.append(attachments)

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

  /**
   * Fetches and renders the bodies of cards as they come near the visible
   * part of the strip. A full message costs Gmail quota several times what
   * the card's headers do, so the bodies of cards nobody scrolls to are never
   * fetched; until then a card shows the snippet.
   */
  private reveal(entries: IntersectionObserverEntry[]): void {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue
      const element = entry.target as HTMLElement
      this.observer.unobserve(element)
      const mounted = this.mounted.get(element.dataset.key ?? '')
      if (!mounted || mounted.body || mounted.fetching) continue
      void this.fetchBody(mounted)
    }
  }

  private async fetchBody(mounted: Mounted): Promise<void> {
    const { card, element } = mounted
    mounted.fetching = true
    let content: MessageContent
    try {
      content = await backend.message(card.account, card.messageId)
    } catch (err) {
      // The snippet stays; the card is tried again when it next comes near.
      if (this.mounted.get(keyOf(card)) === mounted) this.observer.observe(element)
      this.fail(err)
      return
    } finally {
      mounted.fetching = false
    }
    // The strip may have been loaded again while the body was on its way.
    if (this.mounted.get(keyOf(card)) !== mounted) return
    this.showBody(mounted, content)
  }

  private showBody(mounted: Mounted, content: MessageContent): void {
    const { element } = mounted
    const container = element.querySelector<HTMLElement>('.card-body')!
    const remote = content.body.kind === 'html' && content.body.remoteImages
    // A sender the user chose to trust has their images shown from the start.
    const trusted = remote && this.imageSenders.has(mounted.card.from.email)
    mounted.body = mountBody(container, content.body, (url) => this.openLink(url), trusted)
    const scrollbar = new OverlayScrollbar(container, container.parentElement!)
    for (const child of container.children) scrollbar.watch(child)
    if (remote && !trusted) element.querySelector<HTMLElement>('.card-images')!.hidden = false
    mounted.attachments = content.attachments
    if (content.attachments.length > 0) {
      const button = element.querySelector<HTMLElement>('.tool-attachments')!
      const label = `添付ファイル ${content.attachments.length} 件`
      button.innerHTML = ICONS.attachment
      const count = document.createElement('span')
      count.className = 'tool-count'
      count.textContent = String(content.attachments.length)
      button.append(count)
      button.title = label
      button.setAttribute('aria-label', label)
      button.hidden = false
    }
    const lookUp = this.preferences.brandLogos
    this.showTrust(mounted, content.authentication, undefined, lookUp)
    if (lookUp) void this.fetchBrand(mounted, content.authentication)
  }

  private showTrust(
    mounted: Mounted,
    authentication: Authentication,
    brand: Brand | null | undefined,
    lookUp: boolean,
  ): void {
    renderTrust(mounted.element.querySelector<HTMLElement>('.card-trust')!, authentication, brand, lookUp)
  }

  /** Looks up the sender's BIMI logo; the checks say how it went either way. */
  private async fetchBrand(mounted: Mounted, authentication: Authentication): Promise<void> {
    const { card, element } = mounted
    let brand: Brand | null
    try {
      brand = await backend.brand(card.account, card.messageId)
    } catch {
      // Nothing to tell beyond what the checks show.
      brand = null
    }
    if (this.mounted.get(keyOf(card)) !== mounted) return
    if (brand === null && !this.preferences.brandLogos) {
      this.showTrust(mounted, authentication, null, false)
      return
    }
    this.showTrust(mounted, authentication, brand, true)
    if (brand?.logo) {
      const logo = element.querySelector<HTMLImageElement>('.card-logo')!
      logo.src = brand.logo
      logo.title = `${brand.domain} のロゴ（${brand.mark === 'cmc' ? 'CMC' : 'VMC'} で確認済み）`
      logo.hidden = false
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
        void this.takeOff(mounted, () => backend.archive(card.account, card.messageIds, this.mailbox))
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
        this.showImages(mounted)
        break
      case 'images-always':
        void this.trustSender(card.from.email, button)
        break
      case 'attachments':
        this.toggleAttachments(mounted, button)
        break
    }
  }

  /** Opens the list of a card's attachments above its button, or closes it. */
  private toggleAttachments(mounted: Mounted, button: HTMLButtonElement): void {
    if (this.attachmentMenu.isOpenFor(button)) {
      this.attachmentMenu.close()
      return
    }
    this.picker.close()
    const { card } = mounted
    this.attachmentMenu.open(button, mounted.attachments, (attachment, action) =>
      this.runAttachment(card, attachment.id, action),
    )
  }

  private async runAttachment(card: Card, attachmentId: string, action: AttachmentAction): Promise<void> {
    if (action === 'open') {
      await backend.openAttachment(card.account, card.messageId, attachmentId)
      return
    }
    const name = await backend.saveAttachment(card.account, card.messageId, attachmentId)
    showNotice(`ダウンロード フォルダに「${name}」を保存しました`)
  }

  /** Loads the images from the network in one card's message. */
  private showImages(mounted: Mounted): void {
    mounted.body?.allowRemoteImages()
    mounted.element.querySelector<HTMLElement>('.card-images')!.hidden = true
  }

  /**
   * Remembers that messages from `email` show their images without asking,
   * and shows them in every card from that sender still asking.
   */
  private async trustSender(email: string, button: HTMLButtonElement): Promise<void> {
    button.disabled = true
    try {
      await this.imageSenders.allow(email)
    } catch (err) {
      showNotice(errorMessage(err))
      return
    } finally {
      button.disabled = false
    }
    for (const mounted of this.mounted.values()) {
      const asking = !mounted.element.querySelector<HTMLElement>('.card-images')!.hidden
      if (asking && this.imageSenders.has(mounted.card.from.email)) this.showImages(mounted)
    }
  }

  /** Takes the card off the strip, and puts it back where it was if `action` fails. */
  private async takeOff(mounted: Mounted, action: () => Promise<void>): Promise<void> {
    const { element, card } = mounted
    this.attachmentMenu.close()
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
      await backend.applyLabel(account, mounted.card.messageIds, label.id, this.mailbox)
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
      [...byAccount].map(([account, ids]) => backend.archive(account, ids, this.mailbox)),
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
