// The sidebar beside the strip, and the button that shows and hides it. A wide
// window keeps it open beside the cards, which slide under it as the strip
// scrolls; a narrow one opens it over the cards and closes it once a view is
// picked. Whether the user last hid it is remembered on this machine only.

const NARROW = window.matchMedia('(max-width: 759px)')
const STORAGE_KEY = 'mailcrew.sidebar'

function stored(): boolean | null {
  try {
    const value = localStorage.getItem(STORAGE_KEY)
    return value === null ? null : value === 'open'
  } catch {
    return null
  }
}

function store(open: boolean): void {
  try {
    localStorage.setItem(STORAGE_KEY, open ? 'open' : 'closed')
  } catch {
    // Remembering is a convenience; the sidebar works without it.
  }
}

export class Sidebar {
  private readonly inbox = document.querySelector<HTMLElement>('#inbox')!
  private readonly toggle = document.querySelector<HTMLButtonElement>('#toggle-sidebar')!
  private open: boolean

  constructor() {
    this.open = NARROW.matches ? false : (stored() ?? true)
    this.toggle.addEventListener('click', () => {
      this.set(!this.open)
      if (!NARROW.matches) store(this.open)
    })
    NARROW.addEventListener('change', () => this.set(NARROW.matches ? false : (stored() ?? true)))
    this.set(this.open)
  }

  /** A view was picked; over the cards, the sidebar has done its job. */
  picked(): void {
    if (NARROW.matches) this.set(false)
  }

  private set(open: boolean): void {
    this.open = open
    this.inbox.classList.toggle('sidebar-open', open)
    this.toggle.setAttribute('aria-expanded', String(open))
  }
}
