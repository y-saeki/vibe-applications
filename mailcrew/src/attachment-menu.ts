// The list that opens from a card's attachment button: each file's name and
// size, with 開く for the kinds Mail Crew opens and 保存 for all of them. It
// opens above the button, as the tag picker does, and closes on Escape, a
// click outside, or once a file has been opened or saved.

import type { Attachment } from './backend'

export type AttachmentAction = 'open' | 'save'

/** Does what a row's button asks; rejects with what went wrong. */
export type RunAttachment = (attachment: Attachment, action: AttachmentAction) => Promise<void>

const LABELS: Record<AttachmentAction, { idle: string; busy: string }> = {
  open: { idle: '開く', busy: '開いています…' },
  save: { idle: '保存', busy: '保存しています…' },
}

/** A size in bytes as people read it. */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  const units = ['KB', 'MB', 'GB']
  let value = bytes / 1024
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`
}

export class AttachmentMenu {
  private readonly root = document.querySelector<HTMLElement>('#attachment-menu')!
  private readonly list = document.querySelector<HTMLUListElement>('#attachment-list')!
  private anchor: HTMLElement | null = null

  constructor(private readonly onError: (err: unknown) => void) {
    this.root.addEventListener('keydown', (event) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      this.close(true)
    })
    document.addEventListener(
      'pointerdown',
      (event) => {
        if (this.root.hidden) return
        const target = event.target as Node
        if (this.root.contains(target) || this.anchor?.contains(target)) return
        this.close()
      },
      true,
    )
  }

  get isOpen(): boolean {
    return !this.root.hidden
  }

  /** Whether the menu is open for this button. */
  isOpenFor(anchor: HTMLElement): boolean {
    return this.isOpen && this.anchor === anchor
  }

  /** Opens above `anchor` with `attachments`; `run` does what a row's button asks. */
  open(anchor: HTMLElement, attachments: Attachment[], run: RunAttachment): void {
    this.close()
    this.anchor = anchor
    anchor.setAttribute('aria-expanded', 'true')
    this.list.replaceChildren(...attachments.map((attachment) => this.row(attachment, run)))
    this.root.hidden = false
    this.place(anchor)
    this.list.querySelector('button')?.focus()
  }

  /** Closes the menu, and gives the focus back to its button when `refocus` says so. */
  close(refocus = false): void {
    if (this.root.hidden) return
    this.root.hidden = true
    const anchor = this.anchor
    this.anchor = null
    anchor?.setAttribute('aria-expanded', 'false')
    if (refocus) anchor?.focus()
  }

  private row(attachment: Attachment, run: RunAttachment): HTMLElement {
    const row = document.createElement('li')
    row.className = 'attachment-row'
    const name = document.createElement('span')
    name.className = 'attachment-name'
    name.textContent = attachment.name
    name.title = attachment.name
    const size = document.createElement('span')
    size.className = 'attachment-size'
    size.textContent = formatSize(attachment.size)
    const actions = document.createElement('span')
    actions.className = 'attachment-actions'
    // Programs and the like are never opened from here, only saved.
    const kinds: AttachmentAction[] = attachment.openable ? ['open', 'save'] : ['save']
    for (const action of kinds) actions.append(this.button(attachment, action, run))
    row.append(name, size, actions)
    return row
  }

  private button(attachment: Attachment, action: AttachmentAction, run: RunAttachment): HTMLButtonElement {
    const { idle, busy } = LABELS[action]
    const button = document.createElement('button')
    button.type = 'button'
    button.className = 'link-button'
    button.dataset.action = action
    button.textContent = idle
    button.setAttribute('aria-label', `${attachment.name} を${idle}`)
    button.addEventListener('click', async () => {
      // Fetching the file from Gmail can take a few seconds.
      button.disabled = true
      button.textContent = busy
      const anchor = this.anchor
      try {
        await run(attachment, action)
        if (this.anchor === anchor) this.close(true)
      } catch (err) {
        this.onError(err)
      } finally {
        button.disabled = false
        button.textContent = idle
      }
    })
    return button
  }

  /** Above the button, kept inside the window. */
  private place(anchor: HTMLElement): void {
    const box = anchor.getBoundingClientRect()
    const width = this.root.offsetWidth
    const height = this.root.offsetHeight
    const margin = 8
    const left = Math.min(Math.max(margin, box.left), window.innerWidth - width - margin)
    const top = Math.max(margin, box.top - height - margin)
    this.root.style.left = `${left}px`
    this.root.style.top = `${top}px`
  }
}
