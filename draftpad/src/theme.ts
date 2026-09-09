// Resolves the `theme` setting (system / light / dark) to the actual look and
// keeps following the OS while "system" is selected.

import type { Theme } from './state'

export type Resolved = 'light' | 'dark'

const query = window.matchMedia('(prefers-color-scheme: dark)')

export function resolveTheme(theme: Theme): Resolved {
  if (theme === 'system') return query.matches ? 'dark' : 'light'
  return theme
}

/** Applies `theme` and calls `onChange` now and whenever the resolved look changes. */
export class ThemeController {
  private theme: Theme
  private resolved: Resolved

  constructor(theme: Theme, private readonly onChange: (resolved: Resolved) => void) {
    this.theme = theme
    this.resolved = resolveTheme(theme)
    this.apply()
    query.addEventListener('change', () => {
      if (this.theme === 'system') this.update()
    })
  }

  set(theme: Theme): void {
    this.theme = theme
    this.update()
  }

  private update(): void {
    const next = resolveTheme(this.theme)
    if (next === this.resolved) return
    this.resolved = next
    this.apply()
  }

  private apply(): void {
    document.documentElement.dataset.theme = this.resolved
    this.onChange(this.resolved)
  }
}
