// The list that opens from a card's "タグ" button: the labels of the account
// the card is in, narrowed by what is typed, with a row to create a new one
// when nothing matches.

import type { Label } from './backend'

export type TagChoice = { kind: 'existing'; label: Label } | { kind: 'new'; name: string }

interface Row {
  text: string
  choice: TagChoice
}

/** The rows for a filter: matching labels first, then "create" if the name is new. */
export function tagRows(labels: Label[], filter: string): Row[] {
  const query = filter.trim()
  const lower = query.toLowerCase()
  const rows: Row[] = labels
    .filter((label) => label.name.toLowerCase().includes(lower))
    .map((label) => ({ text: label.name, choice: { kind: 'existing', label } }))
  if (query && !labels.some((label) => label.name.toLowerCase() === lower)) {
    rows.push({ text: `「${query}」を作成`, choice: { kind: 'new', name: query } })
  }
  return rows
}

export class TagPicker {
  private readonly root = document.querySelector<HTMLElement>('#tag-picker')!
  private readonly filter = document.querySelector<HTMLInputElement>('#tag-filter')!
  private readonly list = document.querySelector<HTMLUListElement>('#tag-list')!
  private readonly labels = new Map<string, Label[]>()
  private account = ''
  private rows: Row[] = []
  private active = 0
  private resolve: ((choice: TagChoice | null) => void) | null = null
  private anchor: HTMLElement | null = null

  constructor() {
    this.filter.addEventListener('input', () => {
      this.active = 0
      this.render()
    })
    this.filter.addEventListener('keydown', (event) => {
      if (event.isComposing) return
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault()
        if (this.rows.length === 0) return
        const step = event.key === 'ArrowDown' ? 1 : -1
        this.active = (this.active + step + this.rows.length) % this.rows.length
        this.render()
      } else if (event.key === 'Enter') {
        event.preventDefault()
        const row = this.rows[this.active]
        if (row) this.finish(row.choice)
      } else if (event.key === 'Escape') {
        event.preventDefault()
        this.finish(null)
      }
    })
    document.addEventListener(
      'pointerdown',
      (event) => {
        if (this.root.hidden) return
        const target = event.target as Node
        if (this.root.contains(target) || this.anchor?.contains(target)) return
        this.finish(null)
      },
      true,
    )
  }

  get isOpen(): boolean {
    return !this.root.hidden
  }

  /** Replaces one account's labels, keeping the picker open if it is. */
  setLabels(account: string, labels: Label[]): void {
    this.labels.set(account, labels)
    if (this.isOpen && account === this.account) this.render()
  }

  /** Opens next to `anchor` with `account`'s labels and resolves with what was picked, or null. */
  open(anchor: HTMLElement, account: string): Promise<TagChoice | null> {
    this.finish(null)
    this.anchor = anchor
    this.account = account
    this.filter.value = ''
    this.active = 0
    this.render()
    this.root.hidden = false
    this.place(anchor)
    this.filter.focus()
    return new Promise((resolve) => {
      this.resolve = resolve
    })
  }

  close(): void {
    this.finish(null)
  }

  private finish(choice: TagChoice | null): void {
    const resolve = this.resolve
    this.resolve = null
    this.root.hidden = true
    this.anchor = null
    resolve?.(choice)
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

  private render(): void {
    this.rows = tagRows(this.labels.get(this.account) ?? [], this.filter.value)
    this.list.replaceChildren(
      ...this.rows.map((row, index) => {
        const item = document.createElement('li')
        item.className = row.choice.kind === 'new' ? 'tag-row tag-row-new' : 'tag-row'
        item.setAttribute('role', 'option')
        item.setAttribute('aria-selected', String(index === this.active))
        item.textContent = row.text
        item.addEventListener('pointerdown', (event) => event.preventDefault())
        item.addEventListener('click', () => this.finish(row.choice))
        return item
      }),
    )
    if (this.rows.length === 0) {
      const item = document.createElement('li')
      item.className = 'tag-row tag-row-none'
      item.textContent = 'タグがありません'
      this.list.append(item)
    }
  }
}
