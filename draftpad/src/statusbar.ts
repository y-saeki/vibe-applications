// Status bar: the always-on-top toggle and the gear button that opens
// Preferences. The counts and the language selector are in the bar above the
// pane (src/pane-bars.ts).

export interface StatusBarHandlers {
  onAlwaysOnTopChange: (enabled: boolean) => void
  onOpenPreferences: () => void
}

export class StatusBar {
  private readonly alwaysOnTop: HTMLButtonElement

  constructor(root: HTMLElement, handlers: StatusBarHandlers) {
    const q = <T extends Element>(selector: string) => root.querySelector(selector) as T
    this.alwaysOnTop = q('#always-on-top')

    // A button does not flip itself the way a checkbox does: it reports the
    // state it is asking for, and setAlwaysOnTop() writes back what the store
    // settled on.
    this.alwaysOnTop.addEventListener('click', () => handlers.onAlwaysOnTopChange(!this.pressed))
    q<HTMLButtonElement>('#open-preferences').addEventListener('click', () => handlers.onOpenPreferences())
  }

  setAlwaysOnTop(enabled: boolean): void {
    this.alwaysOnTop.setAttribute('aria-pressed', String(enabled))
  }

  private get pressed(): boolean {
    return this.alwaysOnTop.getAttribute('aria-pressed') === 'true'
  }
}
