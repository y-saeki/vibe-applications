// The views in the sidebar: the inbox and what あとで読む set aside, across
// every account, then each custom view the user made in the settings. They are
// a vertical tab list, and the toolbar names the one that is selected.

import type { View } from './backend'
import { ICONS } from './icons'
import type { Filter } from './inbox'

/** The id the tab for the inbox goes by. */
export const INBOX_VIEW = ''
/** The id the tab for what あとで読む set aside goes by. No saved view can have it. */
export const SET_ASIDE_VIEW = '@set-aside'

const FIXED_TABS = [
  { id: INBOX_VIEW, name: '受信トレイ', icon: ICONS.inbox },
  { id: SET_ASIDE_VIEW, name: 'あとで読む', icon: ICONS.later },
]

/** What the strip fetches for the tab with `id`, given the views and who is signed in now. */
export function filterFor(id: string, views: View[], accounts: string[]): Filter {
  if (id === SET_ASIDE_VIEW) return { mailbox: 'setAside', accounts: [], query: '', custom: false }
  const view = views.find((known) => known.id === id)
  if (!view) return { mailbox: 'inbox', accounts: [], query: '', custom: false }
  const chosen = view.accounts.filter((account) => accounts.includes(account))
  // Every account the view was made for has been removed since.
  if (view.accounts.length > 0 && chosen.length === 0) return null
  return { mailbox: 'inbox', accounts: chosen, query: view.query.trim(), custom: true }
}

/** Whether a tab with `id` is still there once `views` are the saved ones. */
export function tabExists(id: string, views: View[]): boolean {
  return FIXED_TABS.some((tab) => tab.id === id) || views.some((view) => view.id === id)
}

/** One line on what a view shows, for the settings. */
export function describeView(view: View): string {
  const accounts = view.accounts.length === 0 ? 'すべてのアカウント' : view.accounts.join('、')
  const query = view.query.trim()
  return query ? `${accounts} ・ ${query}` : accounts
}

export class ViewTabs {
  private readonly root = document.querySelector<HTMLElement>('#views')!
  private readonly title = document.querySelector<HTMLElement>('#view-title')!

  constructor(onSelect: (id: string) => void) {
    this.root.addEventListener('click', (event) => {
      const tab = (event.target as Element).closest<HTMLButtonElement>('button[data-view]')
      if (!tab || tab.getAttribute('aria-selected') === 'true') return
      onSelect(tab.dataset.view ?? INBOX_VIEW)
    })
  }

  render(views: View[], active: string): void {
    const all = [...FIXED_TABS, ...views.map((view) => ({ ...view, icon: ICONS.view }))]
    const tabs = all.map((view) => {
      const tab = document.createElement('button')
      tab.type = 'button'
      tab.className = 'view-tab'
      tab.setAttribute('role', 'tab')
      tab.setAttribute('aria-selected', String(view.id === active))
      tab.dataset.view = view.id
      tab.innerHTML = view.icon
      const name = document.createElement('span')
      name.className = 'view-tab-name'
      name.textContent = view.name
      tab.append(name)
      return tab
    })
    this.root.replaceChildren(...tabs)
    this.title.textContent = all.find((view) => view.id === active)?.name ?? '受信トレイ'
  }
}
