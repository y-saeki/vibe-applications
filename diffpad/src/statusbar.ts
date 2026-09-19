// Status bar: how many chunks differ and the two buttons that step through
// them, the diff mode selector, the always-on-top toggle and the gear button
// that opens Preferences.

import type { DiffMode } from './state'

export interface StatusBarHandlers {
  onDiffModeChange: (mode: DiffMode) => void
  onPreviousDiff: () => void
  onNextDiff: () => void
  onAlwaysOnTopChange: (enabled: boolean) => void
  onOpenPreferences: () => void
}

/** What the count reads while the two panes say the same thing. */
const NO_DIFF = '差異なし'

export class StatusBar {
  private readonly count: HTMLElement
  private readonly previous: HTMLButtonElement
  private readonly next: HTMLButtonElement
  private readonly diffMode: HTMLSelectElement
  private readonly alwaysOnTop: HTMLButtonElement

  constructor(root: HTMLElement, handlers: StatusBarHandlers, platform: string) {
    const q = <T extends Element>(selector: string) => root.querySelector(selector) as T
    this.count = q('#status-diff')
    this.previous = q('#previous-diff')
    this.next = q('#next-diff')
    this.diffMode = q('#diff-mode-select')
    this.alwaysOnTop = q('#always-on-top')

    // The tooltips carry the shortcut in the form the platform writes it.
    const shift = platform === 'macos' ? '⇧ F7' : 'Shift+F7'
    this.previous.title = `前の差異 (${shift})`
    this.next.title = '次の差異 (F7)'

    this.previous.addEventListener('click', () => handlers.onPreviousDiff())
    this.next.addEventListener('click', () => handlers.onNextDiff())
    this.diffMode.addEventListener('change', () => handlers.onDiffModeChange(this.diffMode.value as DiffMode))
    // A button does not flip itself the way a checkbox does: it reports the
    // state it is asking for, and setAlwaysOnTop() writes back what the store
    // settled on.
    this.alwaysOnTop.addEventListener('click', () => handlers.onAlwaysOnTopChange(!this.pressed))
    q<HTMLButtonElement>('#open-preferences').addEventListener('click', () => handlers.onOpenPreferences())
  }

  /** Writes the chunk count, and turns the two step buttons off while there is nothing to step to. */
  setDiffCount(count: number): void {
    const reading = count === 0 ? NO_DIFF : `差異 ${count} 箇所`
    if (this.count.textContent !== reading) this.count.textContent = reading
    this.previous.disabled = count === 0
    this.next.disabled = count === 0
  }

  setDiffMode(mode: DiffMode): void {
    this.diffMode.value = mode
  }

  setAlwaysOnTop(enabled: boolean): void {
    this.alwaysOnTop.setAttribute('aria-pressed', String(enabled))
  }

  private get pressed(): boolean {
    return this.alwaysOnTop.getAttribute('aria-pressed') === 'true'
  }
}
