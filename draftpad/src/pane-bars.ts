// The bar above each pane: the language selector, the character and line
// counts, and the button that opens the compare pane or closes this one. The
// right pane's bar holds the diff unit selector and the +N / −N figures in
// place of the language selector, which serves both panes from the left one.
//
// The two bars are in the page from the start; the right one is hidden while
// there is one pane, and shown with the second.

import type { DiffStat, Side } from './editor'
import { LANGUAGES } from './languages'
import type { DiffMode } from './state'

export interface PaneBarHandlers {
  onLanguageChange: (id: string) => void
  onDiffModeChange: (mode: DiffMode) => void
  onOpenCompare: () => void
  onClosePane: (side: Side) => void
}

export class PaneBars {
  private readonly language: HTMLSelectElement
  private readonly diffMode: HTMLSelectElement
  private readonly chars: Record<Side, HTMLElement>
  private readonly lines: Record<Side, HTMLElement>
  private readonly added: HTMLElement
  private readonly removed: HTMLElement
  private readonly openCompare: HTMLButtonElement
  private readonly closeA: HTMLButtonElement
  /** The right pane's head and foot, which come and go with the pane. */
  private readonly right: HTMLElement[]

  /**
   * @param root the element holding both bars
   * @param handlers what to do when a control is worked
   */
  constructor(root: HTMLElement, handlers: PaneBarHandlers) {
    const q = <T extends Element>(selector: string) => root.querySelector(selector) as T
    this.language = q('#language-select')
    this.diffMode = q('#diff-mode-select')
    this.chars = { a: q('#status-chars'), b: q('#status-chars-b') }
    this.lines = { a: q('#status-lines'), b: q('#status-lines-b') }
    this.added = q('#status-diff .diff-added')
    this.removed = q('#status-diff .diff-removed')
    this.openCompare = q('#open-compare')
    this.closeA = q('#close-a')
    this.right = [q('#pane-head-b'), q('#pane-foot-b')]

    for (const lang of LANGUAGES) {
      const option = document.createElement('option')
      option.value = lang.id
      option.textContent = lang.label
      this.language.append(option)
    }
    this.language.addEventListener('change', () => handlers.onLanguageChange(this.language.value))
    this.diffMode.addEventListener('change', () => handlers.onDiffModeChange(this.diffMode.value as DiffMode))
    this.openCompare.addEventListener('click', () => handlers.onOpenCompare())
    this.closeA.addEventListener('click', () => handlers.onClosePane('a'))
    q<HTMLButtonElement>('#close-b').addEventListener('click', () => handlers.onClosePane('b'))
  }

  setCounts(side: Side, chars: number, lines: number): void {
    this.chars[side].textContent = `${chars} 文字`
    this.lines[side].textContent = `${lines} 行`
  }

  setLanguage(id: string): void {
    this.language.value = id
  }

  setDiffMode(mode: DiffMode): void {
    this.diffMode.value = mode
  }

  /** Writes the lines the right pane adds and takes out, as +N and −N. */
  setDiffStat({ added, removed }: DiffStat): void {
    this.added.textContent = `+${added}`
    // U+2212, the minus sign: a hyphen is too short to read as one beside +.
    this.removed.textContent = `−${removed}`
  }

  /**
   * Shows the right pane's bar, or hides it, and turns the left pane's button
   * from the one that opens the compare pane into the one that closes the
   * left pane, or back.
   */
  setCompare(open: boolean): void {
    for (const element of this.right) element.hidden = !open
    this.openCompare.hidden = open
    this.closeA.hidden = !open
  }
}
