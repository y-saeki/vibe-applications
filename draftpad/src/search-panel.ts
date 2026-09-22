// The parts of the search panel CodeMirror does not build: how many matches the
// current query has and which one is selected, replacing within the selected
// range alone, how many matches a pass replaced, the names the buttons lose
// when style.css collapses their text into an icon, and turning the buttons off
// while there is nothing for them to act on.
//
// CodeMirror offers no say over the panel's markup short of replacing the whole
// panel, so this reaches into the one it built instead. Three elements are
// draftpad's own — the two counts and the button between 置換 and すべて — and
// the rest is attributes, the disabled flag, and where the keyboard goes.

import { getSearchQuery, searchPanelOpen, type SearchQuery } from '@codemirror/search'
import type { EditorState, Extension, Text, Transaction } from '@codemirror/state'
import { EditorView, ViewPlugin, type ViewUpdate } from '@codemirror/view'

/** How long a burst of typing runs before the matches are counted again. */
const RECOUNT_DELAY_MS = 100

/** The classes on the two spans this puts into the panel; style.css places them. */
const COUNT_CLASS = 'cm-search-count'
const REPLACED_CLASS = 'cm-replace-count'

/** What the count reads when the query finds nothing, an unfinished regular
 *  expression included. A draft is a place to keep typing, so a query that does
 *  not parse says the same thing as one that simply misses. */
const NO_MATCH = '一致なし'

/** What the replace field's tail reads after a pass over several matches. */
const replacedReading = (count: number): string => `${count} 件置換`

/** The buttons CodeMirror builds that need a match to do anything. */
const NEEDS_A_MATCH = ['prev', 'next', 'replace', 'replaceAll']

/** The name, the word and the tooltip of the button draftpad adds, which
 *  replaces within the selected range rather than across the whole draft. The
 *  word is as short as the two it stands beside; the tooltip says the rest. */
const IN_SELECTION = 'replaceSelection'
const IN_SELECTION_LABEL = '選択範囲'
const IN_SELECTION_TITLE = '選択範囲の中だけをすべて置換'

/** Every button that changes the draft, in the order the panel holds them. */
const REPLACES = ['replace', IN_SELECTION, 'replaceAll']

/** The userEvent a pass over several matches is dispatched under; CodeMirror's
 *  すべて uses it too, so one reading covers both. */
const REPLACED_ALL = 'input.replace.all'

interface Match {
  from: number
  to: number
}

/**
 * What a search cursor actually hands back. CodeMirror types it as the range
 * alone, while the value also carries the RegExpExecArray of a regular
 * expression match and the flag that marks a range which covers more than the
 * match itself (a character that normalizes to several).
 */
interface CursorMatch extends Match {
  match?: RegExpExecArray
  precise?: boolean
}

/** Reads a query's cursor as what it holds rather than as what it promises. */
function walk(cursor: Iterator<Match>): Iterator<CursorMatch> {
  return cursor as Iterator<CursorMatch>
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

/**
 * Every match of `query` that sits whole inside `from`–`to`, in document order,
 * each with what it would be replaced by.
 *
 * The range goes to the cursor rather than to `SearchQuery.test`, the filter
 * CodeMirror also offers. Its multi-line cursor — the one a query holding `\n`
 * is given — leaves its read position where it is when the filter turns a match
 * down, so it finds that same match again until it gives up and reports the end
 * of the document. One match before the range is then enough to lose every
 * match inside it. A cursor asked for the range has no such trouble.
 */
function replacementsIn(query: SearchQuery, state: EditorState, from: number, to: number): { from: number; to: number; insert: string }[] {
  const changes: { from: number; to: number; insert: string }[] = []
  const cursor = walk(query.getCursor(state, from, to))
  for (let step = cursor.next(); !step.done; step = cursor.next()) {
    const { from: start, to: end, match, precise } = step.value
    // A range that covers more than the match would take the rest with it.
    if (precise === false) continue
    changes.push({ from: start, to: end, insert: replacementFor(query, match) })
  }
  return changes
}

/**
 * The text one match is replaced with.
 *
 * CodeMirror works this out inside the command it does not let a range into, so
 * the rules are written out here to match it: a query that is not literal reads
 * `\n`, `\r` and `\t` in the replacement as those characters, and a regular
 * expression one also reads `$1` … `$9`, `$&` and `$$`.
 *
 * @param query the query as the panel has it
 * @param match the regular expression's match, where there is one
 * @returns what goes in place of the match
 */
function replacementFor(query: SearchQuery, match: RegExpExecArray | undefined): string {
  const text = query.literal
    ? query.replace
    : query.replace.replace(/\\([nrt\\])/g, (_, ch: string) => (ch === 'n' ? '\n' : ch === 'r' ? '\r' : ch === 't' ? '\t' : '\\'))
  if (!match) return text
  return text.replace(/\$([$&]|\d+)/g, (whole, group: string) => {
    if (group === '&') return match[0]
    if (group === '$') return '$'
    // "$12" is group 12 where the expression has one, and group 1 followed by a
    // 2 where it does not. A group that did not take part stands for nothing,
    // the way it does in String.replace.
    for (let digits = group.length; digits > 0; digits--) {
      const n = Number(group.slice(0, digits))
      if (n > 0 && n < match.length) return (match[n] ?? '') + group.slice(digits)
    }
    return whole
  })
}

/**
 * Replaces every match that falls inside the selected range, and nothing
 * outside it, in one entry in the undo history.
 *
 * @param view the editor whose panel the button was pressed in
 */
function replaceInSelection(view: EditorView): void {
  const { state } = view
  const { from, to } = state.selection.main
  if (from === to || state.readOnly) return
  const query = getSearchQuery(state)
  if (!query.valid) return
  const changes = replacementsIn(query, state, from, to)
  if (changes.length === 0) return
  view.dispatch({
    changes,
    effects: EditorView.announce.of(`${state.phrase('replaced $ matches', changes.length)}.`),
    userEvent: REPLACED_ALL,
  })
}

/** How many ranges of the draft a transaction rewrote. */
function changeCount(tr: Transaction): number {
  let count = 0
  tr.changes.iterChanges(() => {
    count++
  })
  return count
}

class SearchPanelExtras {
  private panel: HTMLElement | null = null
  private count: HTMLElement | null = null
  private replacedBox: HTMLElement | null = null
  private matches: Match[] = []
  /** The query and document the matches above were taken from. */
  private counted: { query: SearchQuery; doc: Text } | null = null
  /** The last pass over several matches, and the draft it left behind. */
  private replaced: { count: number; doc: Text; query: SearchQuery } | null = null
  private timer: ReturnType<typeof setTimeout> | undefined

  constructor(private readonly view: EditorView) {
    if (searchPanelOpen(view.state)) this.sync(0)
  }

  update(update: ViewUpdate): void {
    if (!searchPanelOpen(update.state)) {
      if (searchPanelOpen(update.startState)) this.forget()
      return
    }
    for (const tr of update.transactions) {
      if (!tr.isUserEvent(REPLACED_ALL)) continue
      // Counted off the transaction rather than off the matches, so that the
      // figure is what the draft actually took, whichever button ran the pass.
      this.replaced = { count: changeCount(tr), doc: update.state.doc, query: getSearchQuery(update.state) }
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
    this.replacedBox = null
    this.matches = []
    this.counted = null
    this.replaced = null
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
    // A panel closed and reopened is a new element, and new spans with it.
    if (panel !== this.panel) {
      this.panel = panel
      this.count = adopt(panel, this.view)
      this.replacedBox = panel.querySelector<HTMLElement>(`.${REPLACED_CLASS}`)
      this.counted = null
    }

    const { state } = this.view
    const query = getSearchQuery(state)
    if (!this.counted || this.counted.doc !== state.doc || !this.counted.query.eq(query)) {
      this.matches = query.valid ? matchesOf(query, state) : []
      this.counted = { query, doc: state.doc }
    }
    // The figure stands for one pass over one draft: an edit or a new query
    // after it would leave a number that no longer counts anything.
    if (this.replaced && (this.replaced.doc !== state.doc || !this.replaced.query.eq(query))) this.replaced = null

    if (this.count) this.count.textContent = this.reading(query)
    if (this.replacedBox) this.replacedBox.textContent = this.replaced ? replacedReading(this.replaced.count) : ''
    for (const name of NEEDS_A_MATCH) {
      const button = panel.querySelector<HTMLButtonElement>(`button[name="${name}"]`)
      if (button) button.disabled = this.matches.length === 0
    }
    const inSelection = panel.querySelector<HTMLButtonElement>(`button[name="${IN_SELECTION}"]`)
    if (inSelection) inSelection.disabled = !this.selectionHoldsAMatch()
  }

  /**
   * Whether replacing within the selection has anything to do. A range has to
   * be selected, and a match has to sit inside it whole: a range holding
   * nothing but the halves of two matches would come back unchanged, so the
   * button stays off rather than offering a pass that does nothing.
   */
  private selectionHoldsAMatch(): boolean {
    const { from, to } = this.view.state.selection.main
    return from !== to && this.matches.some((match) => match.from >= from && match.to <= to)
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
 * @param view the editor the panel belongs to
 * @returns the span the match count is written into
 */
function adopt(panel: HTMLElement, view: EditorView): HTMLElement {
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

  // The button between 置換 and すべて. It carries the class and the type
  // CodeMirror gives the buttons it builds, so that the panel's styling and its
  // keyboard handling take it for one of them, and it goes into the markup
  // where it is read — which is also where it falls in the tab order. A panel
  // opened on a draft that cannot be edited has no replace row and no すべて to
  // stand beside, and then there is nothing to add here either.
  const replaceEverywhere = panel.querySelector<HTMLButtonElement>('button[name="replaceAll"]')
  if (replaceEverywhere) {
    const inSelection = document.createElement('button')
    inSelection.className = 'cm-button'
    inSelection.type = 'button'
    inSelection.name = IN_SELECTION
    inSelection.textContent = IN_SELECTION_LABEL
    inSelection.title = IN_SELECTION_TITLE
    inSelection.addEventListener('click', () => replaceInSelection(view))
    replaceEverywhere.before(inSelection)
    panel.appendChild(box(REPLACED_CLASS))
  }

  // A press of one of these hands the keyboard back to the draft, so that the
  // undo that follows takes back the replacement rather than the typing in the
  // panel. Only a press: Enter in the replace field replaces as well, and
  // moving the focus there would turn the next Enter into a line break in the
  // draft. These run after the handler that does the replacing, CodeMirror's
  // own included, because they are added later.
  for (const name of REPLACES) {
    const button = panel.querySelector<HTMLButtonElement>(`button[name="${name}"]`)
    button?.addEventListener('click', () => view.focus())
  }

  const count = box(COUNT_CLASS)
  panel.appendChild(count)
  return count
}

/**
 * One of the two figures draftpad writes into the panel.
 *
 * @param className what style.css places it by
 * @returns the span
 */
function box(className: string): HTMLElement {
  const span = document.createElement('span')
  span.className = className
  // Read out on its own; the panel announces a match as it is stepped to, and
  // a pass over several as it finishes.
  span.setAttribute('aria-hidden', 'true')
  return span
}

/** Everything above, as an extension. */
export function searchPanelExtras(): Extension {
  return ViewPlugin.fromClass(SearchPanelExtras)
}
