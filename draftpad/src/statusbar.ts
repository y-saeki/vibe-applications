// Status bar: character/line counts, language selector, always-on-top toggle.

import { LANGUAGES } from './languages'

export interface StatusBarHandlers {
  onLanguageChange: (id: string) => void
  onAlwaysOnTopChange: (enabled: boolean) => void
}

export class StatusBar {
  private readonly chars: HTMLElement
  private readonly lines: HTMLElement
  private readonly language: HTMLSelectElement
  private readonly alwaysOnTop: HTMLInputElement

  constructor(root: HTMLElement, handlers: StatusBarHandlers) {
    const q = <T extends Element>(selector: string) => root.querySelector(selector) as T
    this.chars = q('#status-chars')
    this.lines = q('#status-lines')
    this.language = q('#language-select')
    this.alwaysOnTop = q('#always-on-top')

    for (const lang of LANGUAGES) {
      const option = document.createElement('option')
      option.value = lang.id
      option.textContent = lang.label
      this.language.append(option)
    }
    this.language.addEventListener('change', () => handlers.onLanguageChange(this.language.value))
    this.alwaysOnTop.addEventListener('change', () => handlers.onAlwaysOnTopChange(this.alwaysOnTop.checked))
  }

  setCounts(chars: number, lines: number): void {
    this.chars.textContent = `文字数: ${chars}`
    this.lines.textContent = `行数: ${lines}`
  }

  setLanguage(id: string): void {
    this.language.value = id
  }

  setAlwaysOnTop(enabled: boolean): void {
    this.alwaysOnTop.checked = enabled
  }
}
