// Title bar: the always-on-top toggle and the gear button that opens
// Preferences and, on Windows, the window's own minimize, maximize and close
// buttons, which the frameless window no longer draws for us. The counts and
// the language selector are in the bar above the pane (src/pane-bars.ts).

export interface TitleBarHandlers {
  onAlwaysOnTopChange: (enabled: boolean) => void
  onOpenPreferences: () => void
  onMinimize: () => void
  onToggleMaximize: () => void
  onClose: () => void
}

export class TitleBar {
  private readonly alwaysOnTop: HTMLButtonElement
  private readonly maximize: HTMLButtonElement

  constructor(root: HTMLElement, handlers: TitleBarHandlers) {
    const q = <T extends Element>(selector: string) => root.querySelector(selector) as T
    this.alwaysOnTop = q('#always-on-top')
    this.maximize = q('#window-maximize')

    // A button does not flip itself the way a checkbox does: it reports the
    // state it is asking for, and setAlwaysOnTop() writes back what the store
    // settled on.
    this.alwaysOnTop.addEventListener('click', () => handlers.onAlwaysOnTopChange(!this.pressed))
    q<HTMLButtonElement>('#open-preferences').addEventListener('click', () => handlers.onOpenPreferences())
    // The window's own buttons never take the focus, the way the ones Windows
    // draws do not: the caret stays where it was, and so does the IME that is
    // anchored to it.
    for (const button of root.querySelectorAll('.caption-button')) {
      button.addEventListener('mousedown', (event) => event.preventDefault())
    }
    q<HTMLButtonElement>('#window-minimize').addEventListener('click', () => handlers.onMinimize())
    this.maximize.addEventListener('click', () => handlers.onToggleMaximize())
    q<HTMLButtonElement>('#window-close').addEventListener('click', () => handlers.onClose())
  }

  setAlwaysOnTop(enabled: boolean): void {
    this.alwaysOnTop.setAttribute('aria-pressed', String(enabled))
  }

  /** Turns the maximize button into the one that restores the window, and back. */
  setMaximized(maximized: boolean): void {
    const label = maximized ? '元に戻す' : '最大化'
    this.maximize.dataset.maximized = String(maximized)
    this.maximize.setAttribute('aria-label', label)
    this.maximize.title = label
  }

  private get pressed(): boolean {
    return this.alwaysOnTop.getAttribute('aria-pressed') === 'true'
  }
}
