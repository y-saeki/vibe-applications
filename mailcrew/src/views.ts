// The tabs above the strip: the whole inbox across every account, then each
// custom view the user made in the settings.

import type { View } from './backend'
import type { Filter } from './inbox'

/** The id the tab for the whole inbox goes by. */
export const ALL_VIEW = ''

/** What the strip fetches for a view, given who is signed in now. */
export function filterFor(view: View | undefined, accounts: string[]): Filter {
  if (!view) return { accounts: [], query: '', custom: false }
  const chosen = view.accounts.filter((account) => accounts.includes(account))
  // Every account the view was made for has been removed since.
  if (view.accounts.length > 0 && chosen.length === 0) return null
  return { accounts: chosen, query: view.query.trim(), custom: true }
}

/** One line on what a view shows, for the settings. */
export function describeView(view: View): string {
  const accounts = view.accounts.length === 0 ? 'すべてのアカウント' : view.accounts.join('、')
  const query = view.query.trim()
  return query ? `${accounts} ・ ${query}` : accounts
}

export class ViewTabs {
  private readonly root = document.querySelector<HTMLElement>('#views')!

  constructor(onSelect: (id: string) => void) {
    this.root.addEventListener('click', (event) => {
      const tab = (event.target as Element).closest<HTMLButtonElement>('button[data-view]')
      if (!tab || tab.getAttribute('aria-selected') === 'true') return
      onSelect(tab.dataset.view ?? ALL_VIEW)
    })
  }

  render(views: View[], active: string): void {
    const tabs = [{ id: ALL_VIEW, name: 'すべて' }, ...views].map((view) => {
      const tab = document.createElement('button')
      tab.type = 'button'
      tab.className = 'view-tab'
      tab.setAttribute('role', 'tab')
      tab.setAttribute('aria-selected', String(view.id === active))
      tab.dataset.view = view.id
      tab.textContent = view.name
      return tab
    })
    this.root.replaceChildren(...tabs)
  }
}
