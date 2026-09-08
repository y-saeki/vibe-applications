// Command palette: a filter box over the command table.

import { formatKeys, type Command } from './commands'

const MAX_ROWS = 40

export class Palette {
  private readonly root: HTMLElement
  private readonly input: HTMLInputElement
  private readonly list: HTMLUListElement
  private matches: Command[] = []
  private selected = 0

  constructor(
    root: HTMLElement,
    private readonly commands: () => readonly Command[],
    private readonly platform: string,
    private readonly onClose: () => void,
  ) {
    this.root = root
    this.input = root.querySelector('#palette-input') as HTMLInputElement
    this.list = root.querySelector('#palette-list') as HTMLUListElement

    this.input.addEventListener('input', () => this.render())
    this.input.addEventListener('keydown', (event) => this.onKeyDown(event))
    this.list.addEventListener('click', (event) => {
      const row = (event.target as HTMLElement).closest('li[data-index]')
      if (!row) return
      this.selected = Number((row as HTMLElement).dataset.index)
      void this.runSelected()
    })
    root.addEventListener('mousedown', (event) => {
      if (event.target === root) this.close()
    })
  }

  get isOpen(): boolean {
    return !this.root.hidden
  }

  open(): void {
    this.root.hidden = false
    this.input.value = ''
    this.selected = 0
    this.render()
    this.input.focus()
  }

  close(): void {
    if (this.root.hidden) return
    this.root.hidden = true
    this.onClose()
  }

  private render(): void {
    const query = this.input.value.trim().toLowerCase()
    const all = this.commands()
    this.matches = (query ? all.filter((c) => c.title.toLowerCase().includes(query) || c.id.includes(query)) : all).slice(
      0,
      MAX_ROWS,
    )
    this.selected = Math.min(this.selected, Math.max(0, this.matches.length - 1))
    this.list.replaceChildren()
    if (this.matches.length === 0) {
      const empty = document.createElement('li')
      empty.className = 'palette-empty'
      empty.textContent = '該当するコマンドはありません'
      this.list.append(empty)
      return
    }
    this.matches.forEach((command, index) => {
      const row = document.createElement('li')
      row.dataset.index = String(index)
      row.setAttribute('role', 'option')
      row.setAttribute('aria-selected', String(index === this.selected))
      const title = document.createElement('span')
      title.textContent = command.title
      row.append(title)
      if (command.keys) {
        const keys = document.createElement('span')
        keys.className = 'palette-keys'
        keys.textContent = formatKeys(command.keys, this.platform)
        row.append(keys)
      }
      this.list.append(row)
    })
    this.scrollSelectedIntoView()
  }

  private onKeyDown(event: KeyboardEvent): void {
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault()
        this.move(1)
        break
      case 'ArrowUp':
        event.preventDefault()
        this.move(-1)
        break
      case 'Enter':
        event.preventDefault()
        void this.runSelected()
        break
      case 'Escape':
        event.preventDefault()
        this.close()
        break
    }
  }

  private move(delta: number): void {
    if (this.matches.length === 0) return
    this.selected = (this.selected + delta + this.matches.length) % this.matches.length
    this.list.querySelectorAll('li[data-index]').forEach((row, index) => {
      row.setAttribute('aria-selected', String(index === this.selected))
    })
    this.scrollSelectedIntoView()
  }

  private scrollSelectedIntoView(): void {
    this.list.querySelector('li[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' })
  }

  private async runSelected(): Promise<void> {
    const command = this.matches[this.selected]
    if (!command) return
    this.close()
    await command.run()
  }
}
