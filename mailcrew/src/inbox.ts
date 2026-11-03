// The inbox: one card per thread, newest at the left, each with its toolbar.
// Every action takes the card off the strip at once and puts it back if the
// Rust side reports a failure.

import { backend, errorMessage, isCommandError, type Card, type Label } from './backend'
import { formatDate } from './format'
import { ICONS } from './icons'
import { mountBody, type MountedBody } from './message-frame'
import { showNotice } from './notice'
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
  private readonly picker = new TagPicker()
  private readonly mounted = new Map<string, Mounted>()
  private labels: Label[] = []
  private loadingNow = false
  private readonly observer = new IntersectionObserver((entries) => this.reveal(entries), {
    root: this.strip,
    rootMargin: PRELOAD_MARGIN,
  })

  constructor(private readonly onSignedOut: (message: string) => void) {
    this.strip.addEventListener('click', (event) => this.onClick(event))
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

  show(email: string): void {
    document.querySelector<HTMLElement>('#account')!.textContent = email
    this.root.hidden = false
  }

  hide(): void {
    this.picker.close()
    this.root.hidden = true
    this.clear()
  }

  /** Fetches the newest threads and lays them out from the start of the strip. */
  async load(): Promise<void> {
    if (this.loadingNow) return
    this.loadingNow = true
    this.picker.close()
    this.loading.hidden = false
    try {
      const cards = await backend.inbox()
      this.render(cards)
      this.refreshLabels()
    } catch (err) {
      this.fail(err)
    } finally {
      this.loading.hidden = true
      this.loadingNow = false
    }
  }

  private clear(): void {
    this.observer.disconnect()
    this.mounted.clear()
    this.strip.replaceChildren()
  }

  private render(cards: Card[]): void {
    this.clear()
    for (const card of cards) {
      const element = this.buildCard(card)
      this.mounted.set(card.id, { card, element, body: null })
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

  private refreshLabels(): void {
    backend
      .labels()
      .then((labels) => {
        this.labels = labels
        this.picker.setLabels(labels)
      })
      .catch(() => {
        // The picker asks again when it opens; nothing to say yet.
      })
  }

  private buildCard(card: Card): HTMLElement {
    const element = document.createElement('article')
    element.className = 'card'
    element.dataset.threadId = card.id
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
    if (meta.childElementCount > 0) head.append(meta)

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

    const body = document.createElement('div')
    body.className = 'card-body'
    const snippet = document.createElement('p')
    snippet.className = 'card-snippet'
    snippet.textContent = card.snippet
    body.append(snippet)

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

    element.append(head, images, body, toolbar)
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
      const mounted = this.mounted.get(element.dataset.threadId ?? '')
      if (!mounted || mounted.body) continue
      const container = element.querySelector<HTMLElement>('.card-body')!
      mounted.body = mountBody(container, mounted.card.body, (url) => this.openLink(url))
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
    const mounted = this.mounted.get(element?.dataset.threadId ?? '')
    if (!mounted) return
    switch (button.dataset.action) {
      case 'done':
        void this.takeOff(mounted, () => backend.archive(mounted.card.messageIds))
        break
      case 'later':
        void this.takeOff(mounted, () => backend.setAside(mounted.card.messageIds))
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
    this.mounted.delete(card.id)
    element.remove()
    this.updateCount()
    try {
      await action()
    } catch (err) {
      this.strip.insertBefore(element, next?.isConnected ? next : this.strip.querySelector('.card-end'))
      this.mounted.set(card.id, mounted)
      this.updateCount()
      this.fail(err)
      return
    }
    // The strip shows a page of the inbox at a time; once it has been worked
    // through, the next page is fetched.
    if (this.mounted.size === 0) await this.load()
  }

  private async pickTag(mounted: Mounted, button: HTMLButtonElement): Promise<void> {
    if (this.labels.length === 0) this.refreshLabels()
    const choice: TagChoice | null = await this.picker.open(button)
    if (!choice) return
    await this.takeOff(mounted, async () => {
      const label = choice.kind === 'existing' ? choice.label : await backend.createLabel(choice.name)
      if (choice.kind === 'new') {
        this.labels = [...this.labels.filter((known) => known.id !== label.id), label].sort((a, b) =>
          a.name.localeCompare(b.name),
        )
        this.picker.setLabels(this.labels)
      }
      await backend.applyLabel(mounted.card.messageIds, label.id)
    })
  }

  private async toggleStar(mounted: Mounted, button: HTMLButtonElement): Promise<void> {
    const { card } = mounted
    const starred = !card.starred
    card.starred = starred
    button.setAttribute('aria-pressed', String(starred))
    button.disabled = true
    try {
      await backend.setStarred(card.messageId, card.messageIds, starred)
    } catch (err) {
      card.starred = !starred
      button.setAttribute('aria-pressed', String(!starred))
      this.fail(err)
    } finally {
      button.disabled = false
    }
  }

  /** Archives every card on the strip, then fetches the next page. */
  private async doneAll(button: HTMLButtonElement): Promise<void> {
    const ids = [...this.mounted.values()].flatMap((mounted) => mounted.card.messageIds)
    if (ids.length === 0) return
    button.disabled = true
    try {
      await backend.archive(ids)
    } catch (err) {
      button.disabled = false
      this.fail(err)
      return
    }
    await this.load()
  }

  private fail(err: unknown): void {
    if (isCommandError(err) && err.kind === 'signedOut') {
      this.onSignedOut(err.message)
      return
    }
    showNotice(errorMessage(err))
  }
}
