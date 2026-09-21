// The parts of the search panel CodeMirror does not build: how many matches the
// current query has and which one is selected, the names the buttons lose when
// style.css collapses their text into an icon, and turning the buttons off while
// there is nothing for them to act on.
//
// CodeMirror offers no say over the panel's markup short of replacing the whole
// panel, so this reaches into the one it built instead. Everything here is DOM
// the editor does not own: the count is a span of draftpad's own, and the rest
// is attributes and the disabled flag.

import { getSearchQuery, searchPanelOpen, type SearchQuery } from '@codemirror/search'
import type { EditorState, Extension, Text } from '@codemirror/state'
import { EditorView, ViewPlugin, type ViewUpdate } from '@codemirror/view'

/** How long a burst of typing runs before the matches are counted again. */
const RECOUNT_DELAY_MS = 100

/** The class on the span this puts into the panel; style.css places it. */
const COUNT_CLASS = 'cm-search-count'

/** What the count reads when the query finds nothing, an unfinished regular
 *  expression included. A draft is a place to keep typing, so a query that does
 *  not parse says the same thing as one that simply misses. */
const NO_MATCH = '一致なし'

/** The buttons that need a match to do anything. */
const NEEDS_A_MATCH = ['prev', 'next', 'replace', 'replaceAll']

interface Match {
  from: number
  to: number
}

/** Every match of `query`, in document order. */
function matchesOf(query: SearchQuery, state: EditorState): Match[] {
  const found: Match[] = []
  // The cursor hands back one object it keeps overwriting, so each range is
  // copied out rather than held on to.
  const cursor = query.getCursor(state)
  for (let step = cursor.next(); !step.done; step = cursor.next()) {
    found.push({ from: step.value.from, to: step.value.to })
  }
  return found
}

class SearchPanelExtras {
  private panel: HTMLElement | null = null
  private count: HTMLElement | null = null
  private matches: Match[] = []
  /** The query and document the matches above were taken from. */
  private counted: { query: SearchQuery; doc: Text } | null = null
  private timer: ReturnType<typeof setTimeout> | undefined

  constructor(private readonly view: EditorView) {
    if (searchPanelOpen(view.state)) this.sync(0)
  }

  update(update: ViewUpdate): void {
    if (!searchPanelOpen(update.state)) {
      if (searchPanelOpen(update.startState)) this.forget()
      return
    }
    if (update.docChanged) {
      // Restarted on every keystroke: a long document is scanned once the
      // typing stops rather than once per character.
      this.sync(RECOUNT_DELAY_MS, true)
    } else if (
      !searchPanelOpen(update.startState) ||
      update.selectionSet ||
      !getSearchQuery(update.startState).eq(getSearchQuery(update.state))
    ) {
      this.sync(0)
    }
  }

  destroy(): void {
    this.forget()
  }

  private forget(): void {
    clearTimeout(this.timer)
    this.timer = undefined
    this.panel = null
    this.count = null
    this.matches = []
    this.counted = null
  }

  /**
   * Refreshes the panel after `delay` milliseconds.
   *
   * Even a delay of 0 matters on the update that opens the panel: CodeMirror
   * writes the panel's DOM later in that cycle, so there is nothing to find yet.
   *
   * @param delay how long to wait
   * @param restart whether a wake-up already pending should be pushed back
   */
  private sync(delay: number, restart = false): void {
    if (this.timer !== undefined) {
      if (!restart) return
      clearTimeout(this.timer)
    }
    this.timer = setTimeout(() => {
      this.timer = undefined
      this.refresh()
    }, delay)
  }

  private refresh(): void {
    const panel = this.view.dom.querySelector<HTMLElement>('.cm-panel.cm-search')
    if (!panel) {
      this.forget()
      return
    }
    // A panel closed and reopened is a new element, and a new span with it.
    if (panel !== this.panel) {
      this.panel = panel
      this.count = adopt(panel)
      this.counted = null
    }

    const { state } = this.view
    const query = getSearchQuery(state)
    if (!this.counted || this.counted.doc !== state.doc || !this.counted.query.eq(query)) {
      this.matches = query.valid ? matchesOf(query, state) : []
      this.counted = { query, doc: state.doc }
    }

    if (this.count) this.count.textContent = this.reading(query)
    for (const name of NEEDS_A_MATCH) {
      const button = panel.querySelector<HTMLButtonElement>(`button[name="${name}"]`)
      if (button) button.disabled = this.matches.length === 0
    }
  }

  /** What the count says about the query as it stands. */
  private reading(query: SearchQuery): string {
    if (query.search === '') return ''
    const total = this.matches.length
    if (total === 0) return NO_MATCH
    const { from, to } = this.view.state.selection.main
    const at = this.matches.findIndex((match) => match.from === from && match.to === to)
    // Until one of them is stepped to, there is no current match to number.
    return at === -1 ? `${total} 件` : `${at + 1} / ${total} 件`
  }
}

/**
 * Takes over the panel CodeMirror has just built.
 *
 * @param panel the panel element
 * @returns the span the count is written into
 */
function adopt(panel: HTMLElement): HTMLElement {
  // style.css collapses the text of these two, which is where their accessible
  // names came from. The words are CodeMirror's phrases, so they are read back
  // off the elements rather than repeated here.
  for (const button of panel.querySelectorAll<HTMLButtonElement>('button[name="prev"], button[name="next"]')) {
    const word = button.textContent?.trim() ?? ''
    button.setAttribute('aria-label', word)
    button.title = word
  }
  for (const label of panel.querySelectorAll<HTMLLabelElement>('label')) {
    label.title = label.textContent?.trim() ?? ''
  }

  const count = document.createElement('span')
  count.className = COUNT_CLASS
  // Read out on its own; the panel announces a match as it is stepped to.
  count.setAttribute('aria-hidden', 'true')
  panel.appendChild(count)
  return count
}

/** Everything above, as an extension. */
export function searchPanelExtras(): Extension {
  return ViewPlugin.fromClass(SearchPanelExtras)
}
