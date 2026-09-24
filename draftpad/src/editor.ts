// CodeMirror setup. Everything a setting can change lives in a Compartment so
// it can be swapped at runtime without rebuilding the editor.
//
// The editor is one pane most of the time and two while the compare pane is
// open. One pane is a plain EditorView. Two are a MergeView, which holds an
// EditorView for each side, recomputes the chunks between them on every edit
// on either side (line by line, see linediff.ts) and keeps the two level.
// Opening the compare pane and closing either side rebuild the editor from the
// one form into the other, carrying the surviving text, its caret and every
// setting across. Opening carries the draft's history into the left pane, so
// undo there goes on into what was typed before; the right pane starts with
// none. A pane closed is where undo stops for the pane that goes on, but one
// more undo there puts the closed pane back, with its text, caret and history.

import { closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete'
import { defaultKeymap, history, historyField, historyKeymap, indentWithTab, redo as redoCommand, redoDepth, undo as undoCommand, undoDepth } from '@codemirror/commands'
import { bracketMatching, defaultHighlightStyle, indentOnInput, indentUnit, syntaxHighlighting } from '@codemirror/language'
import { getChunks, MergeView } from '@codemirror/merge'
import { getSearchQuery, highlightSelectionMatches, openSearchPanel, search, searchKeymap, SearchQuery, setSearchQuery } from '@codemirror/search'
import { Compartment, type EditorSelection, EditorState, type Extension, type Text } from '@codemirror/state'
import { drawSelection, dropCursor, EditorView, keymap, type KeyBinding, lineNumbers } from '@codemirror/view'

import { darkTheme } from './dark-theme'
import { indentGuides } from './indent-guides'
import { languageExtension } from './languages'
import { installLineDiff } from './linediff'
import { overlayScrollbar, type OverlayScrollbar } from './overlay-scrollbar'
import { searchPanelExtras } from './search-panel'
import type { DiffMode, IndentStyle, State } from './state'
import { whitespaceMarks } from './whitespace'

/**
 * The left pane is `a` and the right one `b`, as MergeView names them. While
 * there is one pane, it is `a`.
 */
export type Side = 'a' | 'b'

/** The two toggles in the search panel, as they are persisted. */
export interface SearchOptions {
  caseSensitive: boolean
  regexp: boolean
}

/** How many lines the right pane adds to the left one, and how many it takes out, over every chunk. */
export interface DiffStat {
  added: number
  removed: number
}

export interface EditorOptions {
  /** Where the pane, or the two panes, are put. */
  parent: HTMLElement
  /** Where the scrollbar's strip is appended; see src/overlay-scrollbar.ts. */
  host: HTMLElement
  initial: Readonly<State>
  /** Used when `state.fontFamily` is empty. */
  defaultFontFamily: string
  dark: boolean
  onDocChanged: (side: Side) => void
  onSearchOptionsChanged: (options: SearchOptions) => void
  /** Called with what the right pane adds and takes out, whenever the chunks are recomputed. */
  onDiffChanged: (stat: DiffStat) => void
  /** Called once undo has put a closed pane back, so that there are two again. */
  onPaneRestored: () => void
}

/** What a pane is built with: its text, and optionally where the caret is and the history behind it. */
interface PaneStart {
  doc: string | Text
  selection?: EditorSelection
  /** As `historyField` holds it; an empty history when left out. */
  history?: unknown
}

/** The two panes as they were the moment one of them was closed. */
interface ClosedPane {
  side: Side
  /** The pane that was closed, history and all. */
  closed: EditorState
  /** The pane that went on, as it was then. */
  survivor: EditorState
}

/**
 * How long one diff may take before the rest of it is approximated.
 *
 * The merge view's own default bounds the diff by its depth instead
 * (`scanLimit: 500`), which gives up on two versions of a document that differ
 * in a few hundred places and paints them as one chunk. A time budget keeps
 * the precise diff wherever it is affordable, which is every realistic pair of
 * similar texts, and only two unrelated texts of some size run into it. The
 * budget covers the line-level pass and the character-level passes within
 * the chunks together.
 */
const DIFF_TIMEOUT_MS = 500

const phrases = EditorState.phrases.of({
  Find: '検索',
  Replace: '置換',
  next: '次へ',
  previous: '前へ',
  'match case': '大文字小文字を区別',
  regexp: '正規表現',
  replace: '置換',
  // Pairs with 置換 beside it, in a button the panel gives the same width as
  // every other; すべて置換 does not fit there.
  'replace all': 'すべて',
  close: '閉じる',
  'current match': '現在の一致',
  'replaced $ matches': '$ 件を置換しました',
  'replaced match on line $': '$ 行目の一致を置換しました',
  'on line': '行',
  'Go to line': '行へ移動',
  go: '移動',
})

// Multiple selections stay off (EditorState.allowMultipleSelections), so a
// selection holding several ranges collapses to its main one. That makes
// searchKeymap's Mod-Shift-l — select every match of the selection — do
// nothing, so it goes along with the panel's "all" button that style.css
// hides. Mod-d keeps the half that still works: selecting the word under the
// cursor.
const searchBindings = searchKeymap.filter((binding) => binding.key !== 'Mod-Shift-l')

// historyKeymap binds redo to Mod-y everywhere and to Ctrl-Shift-z on Linux
// only, so Windows needs this one; on macOS it repeats the Cmd-Shift-z binding
// historyKeymap already has.
const redoKeymap: KeyBinding[] = [{ key: 'Mod-Shift-z', run: redoCommand, preventDefault: true }]

// The weight goes on the scroller with the family: CodeMirror's own theme
// leaves font-weight alone, so the content inherits it, while the bold the
// highlight styles put on headings and strong text still wins where it applies.
function fontTheme(size: number, family: string, weight: number): Extension {
  return EditorView.theme({
    '&': { fontSize: `${size}px` },
    '.cm-scroller': { fontFamily: family, fontWeight: String(weight) },
  })
}

// With a tab as the unit, CodeMirror indents with tabs and fills any columns
// left over with spaces, so a tab stays one tab width wide either way.
function tabExtension(size: number, style: IndentStyle): Extension {
  return [EditorState.tabSize.of(size), indentUnit.of(style === 'tabs' ? '\t' : ' '.repeat(size))]
}

function colorExtension(dark: boolean): Extension {
  return dark ? darkTheme : syntaxHighlighting(defaultHighlightStyle)
}

function whitespaceExtension(show: boolean): Extension {
  return show ? whitespaceMarks : []
}

function indentGuideExtension(show: boolean): Extension {
  return show ? indentGuides : []
}

// The gutter the view package already draws, rather than a column of
// draftpad's own: it reserves its width from the largest number in the draft,
// renders only what the viewport holds, and leaves a wrapped line one number
// at its top. style.css takes the panel and the rule it comes dressed in off
// again, so that what is left beside the draft is the figures.
function lineNumberExtension(show: boolean): Extension {
  return show ? lineNumbers() : []
}

// The search extension reads these when it builds the initial query, which is
// the one the panel shows the first time it opens. Every later query inherits
// the flags from the one before it, so setting them here is enough to carry the
// toggles over — from the previous run, and into a pane built later.
function searchExtension(options: SearchOptions): Extension {
  return search({ caseSensitive: options.caseSensitive, regexp: options.regexp })
}

function searchOptionsOf(state: EditorState): SearchOptions {
  const query = getSearchQuery(state)
  return { caseSensitive: query.caseSensitive, regexp: query.regexp }
}

function sameSearchOptions(x: SearchOptions, y: SearchOptions): boolean {
  return x.caseSensitive === y.caseSensitive && x.regexp === y.regexp
}

/**
 * How many lines of `doc` lie between `from` and `to`, where `to` is one past
 * the end of the last line — which is past the document itself when that line
 * is its last, as the merge view builds its chunks.
 */
function linesBetween(doc: Text, from: number, to: number): number {
  if (to <= from) return 0
  return doc.lineAt(Math.min(to - 1, doc.length)).number - doc.lineAt(from).number + 1
}

/** The settings that live in a compartment each, by name. */
type Slot = 'language' | 'colors' | 'font' | 'tab' | 'whitespace' | 'indentGuides' | 'lineNumbers'

export class Editor {
  private single: EditorView | null = null
  private merge: MergeView | null = null
  /** The pane that last had the keyboard, which is where a command acts. */
  private activeSide: Side = 'a'
  private scrollbar: OverlayScrollbar | null = null
  /** What the last close took away, for as long as undo can still bring it back. */
  private closedPane: ClosedPane | null = null
  // One compartment serves both panes: each state keeps its own content for
  // it, and a change is dispatched to every pane in turn.
  private readonly compartments: Record<Slot, Compartment> = {
    language: new Compartment(),
    colors: new Compartment(),
    font: new Compartment(),
    tab: new Compartment(),
    whitespace: new Compartment(),
    indentGuides: new Compartment(),
    lineNumbers: new Compartment(),
  }
  /** What each compartment holds, so that a pane built later starts out the same. */
  private readonly current: Record<Slot, Extension>
  private searchOptions: SearchOptions
  private diffMode: DiffMode
  /**
   * The undo key, ahead of the history's own, while undo would put a closed
   * pane back rather than step through a history. The rebuild waits for the
   * key's handling to finish, since it destroys the view handling it.
   */
  private readonly restoreKey: KeyBinding = {
    key: 'Mod-z',
    run: () => {
      if (!this.canRestoreClosedPane) return false
      queueMicrotask(() => this.restoreClosedPane())
      return true
    },
    preventDefault: true,
  }
  private readonly defaultFontFamily: string

  private constructor(
    private readonly options: EditorOptions,
    language: Extension,
  ) {
    const { initial } = options
    this.defaultFontFamily = options.defaultFontFamily
    this.current = {
      language,
      colors: colorExtension(options.dark),
      font: fontTheme(initial.fontSize, this.fontFamily(initial.fontFamily), initial.fontWeight),
      tab: tabExtension(initial.tabSize, initial.indentStyle),
      whitespace: whitespaceExtension(initial.showWhitespace),
      indentGuides: indentGuideExtension(initial.showIndentGuides),
      lineNumbers: lineNumberExtension(initial.showLineNumbers),
    }
    this.searchOptions = { caseSensitive: initial.searchCaseSensitive, regexp: initial.searchRegexp }
    this.diffMode = initial.diffMode
    if (initial.compare) this.buildMerge({ doc: initial.text }, { doc: initial.compareText })
    else this.buildSingle(initial.text)
  }

  static async create(options: EditorOptions): Promise<Editor> {
    return new Editor(options, await languageExtension(options.initial.language))
  }

  // ---- the panes ----------------------------------------------------------

  /** True while the compare pane is open, so there are two panes. */
  get compare(): boolean {
    return this.merge !== null
  }

  /** The pane that last had the keyboard. */
  get activePane(): Side {
    return this.merge ? this.activeSide : 'a'
  }

  /** The text of a pane; empty for the right one while there is no such pane. */
  text(side: Side): string {
    return this.paneView(side)?.state.doc.toString() ?? ''
  }

  lineCount(side: Side): number {
    return this.paneView(side)?.state.doc.lines ?? 0
  }

  /**
   * Opens the compare pane: the draft becomes the left pane, history and all,
   * and the right one starts out as a copy of it, caret included, with no
   * history. The keyboard goes to the new pane, which is where the text to
   * compare against is about to be put.
   */
  openCompare(): void {
    if (this.merge) return
    const { state } = this.single!
    this.closedPane = null
    this.teardown()
    const copy: PaneStart = { doc: state.doc, selection: state.selection }
    this.buildMerge({ ...copy, history: state.field(historyField) }, copy)
    this.activeSide = 'b'
    this.merge!.b.focus()
  }

  /**
   * Closes one of the two panes. The other one goes on as the draft, with its
   * text and caret and a history of its own that starts here. Undoing past
   * that start brings the closed pane back; see `restoreClosedPane`.
   *
   * @param side which pane to close
   */
  closePane(side: Side): void {
    if (!this.merge) return
    const closed = side === 'a' ? this.merge.a.state : this.merge.b.state
    const survivor = side === 'a' ? this.merge.b.state : this.merge.a.state
    this.teardown()
    this.buildSingle(survivor.doc, survivor.selection)
    this.closedPane = { side, closed, survivor }
    this.activeSide = 'a'
    this.single!.focus()
  }

  /**
   * True while undo would put the last closed pane back: every edit made to
   * the draft since, if any, has been undone. The document is compared as
   * well, because the history forgets its oldest steps past a certain depth,
   * and a draft that cannot be undone back to where it was cannot take the
   * old history either.
   */
  private get canRestoreClosedPane(): boolean {
    const closed = this.closedPane
    if (!closed || !this.single) return false
    const { state } = this.single
    return undoDepth(state) === 0 && state.doc.eq(closed.survivor.doc)
  }

  /**
   * Puts the last closed pane back beside the draft, as the undo of closing
   * it. Each pane takes up its history from where it stood at the close, so
   * undoing further goes on into what was typed before it. The keyboard goes
   * to the pane that comes back.
   *
   * @returns whether there was a pane to put back
   */
  private restoreClosedPane(): boolean {
    if (!this.canRestoreClosedPane) return false
    const { side, closed, survivor } = this.closedPane!
    const { selection } = this.single!.state
    const kept: PaneStart = { doc: survivor.doc, selection, history: survivor.field(historyField) }
    const back: PaneStart = { doc: closed.doc, selection: closed.selection, history: closed.field(historyField) }
    this.closedPane = null
    this.teardown()
    if (side === 'a') this.buildMerge(back, kept)
    else this.buildMerge(kept, back)
    this.activeSide = side
    this.paneView(side)!.focus()
    this.options.onPaneRestored()
    return true
  }

  private buildSingle(doc: string | Text, selection?: EditorSelection): void {
    const view = new EditorView({
      state: EditorState.create({ doc, selection, extensions: this.paneExtensions('a') }),
      parent: this.options.parent,
    })
    this.single = view
    // The scroller keeps its size as the draft grows, so the bar is given
    // the content to watch as well.
    this.scrollbar = overlayScrollbar(view.scrollDOM, this.options.host, view.contentDOM)
  }

  private buildMerge(a: PaneStart, b: PaneStart): void {
    installLineDiff()
    // The merge view hands the recomputed chunks to both panes after an edit on
    // either, so listening on one of them is enough.
    const chunks = EditorView.updateListener.of((update) => {
      const before = getChunks(update.startState)?.chunks
      const after = getChunks(update.state)?.chunks
      if (before !== after) this.options.onDiffChanged(this.diffStat())
    })
    const merge = new MergeView({
      a: { doc: a.doc, selection: a.selection, extensions: [this.paneExtensions('a', a.history), chunks] },
      b: { doc: b.doc, selection: b.selection, extensions: this.paneExtensions('b', b.history) },
      parent: this.options.parent,
      // The stripe beside a changed line is drawn by style.css on the line
      // itself, so that the text has the pane's whole width: the merge
      // view's own gutter would take a column out of it.
      gutter: false,
      highlightChanges: this.diffMode === 'char',
      diffConfig: { timeout: DIFF_TIMEOUT_MS },
    })
    this.merge = merge
    // The merge view is what scrolls; the two panes inside it grow with their
    // text, so the bar watches the element that holds them.
    const editors = merge.dom.querySelector<HTMLElement>('.cm-mergeViewEditors') ?? undefined
    this.scrollbar = overlayScrollbar(merge.dom, this.options.host, editors)
    this.options.onDiffChanged(this.diffStat())
  }

  private teardown(): void {
    this.scrollbar?.destroy()
    this.scrollbar = null
    this.single?.destroy()
    this.single = null
    this.merge?.destroy()
    this.merge = null
  }

  /**
   * Everything a pane is made of, from the settings as they stand.
   *
   * @param past a history to start from, as `historyField` held it, in
   *   place of an empty one. It must belong to the document the pane starts
   *   with.
   */
  private paneExtensions(side: Side, past?: unknown): Extension {
    const c = this.compartments
    const v = this.current
    return [
      c.language.of(v.language),
      c.colors.of(v.colors),
      c.font.of(v.font),
      c.tab.of(v.tab),
      c.whitespace.of(v.whitespace),
      c.indentGuides.of(v.indentGuides),
      c.lineNumbers.of(v.lineNumbers),
      history(),
      past === undefined ? [] : historyField.init(() => past),
      drawSelection(),
      dropCursor(),
      indentOnInput(),
      bracketMatching(),
      closeBrackets(),
      highlightSelectionMatches(),
      searchExtension(this.searchOptions),
      // The search panel is one of CodeMirror's bottom panels, at the foot of the pane's own box. In the merge view that box
      // is as tall as the text, and the panel keeps itself in view by being
      // sticky against the scrolling merge view — as long as nothing between
      // the two clips, which style.css sees to. Its height goes on the end of
      // this pane alone, after the last line, so the lines stay level.
      searchPanelExtras(),
      EditorView.lineWrapping,
      EditorView.contentAttributes.of({ spellcheck: 'false', autocorrect: 'off', autocapitalize: 'off' }),
      phrases,
      keymap.of([...closeBracketsKeymap, ...searchBindings, ...redoKeymap, this.restoreKey, ...historyKeymap, ...defaultKeymap, indentWithTab]),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) this.options.onDocChanged(side)
        if (update.focusChanged && update.view.hasFocus) this.activeSide = side
        const before = searchOptionsOf(update.startState)
        const after = searchOptionsOf(update.state)
        if (!sameSearchOptions(before, after)) this.searchOptionsChanged(side, after)
      }),
    ]
  }

  /**
   * The two toggles are one setting, whichever pane's panel they were flipped
   * in: the other pane's query takes the same flags, keeping its own words.
   * The other pane is updated once this update has run its course, as the
   * view asks; it will report the change back through the same path and find
   * nothing left to do.
   */
  private searchOptionsChanged(side: Side, options: SearchOptions): void {
    this.searchOptions = options
    this.options.onSearchOptionsChanged(options)
    const other = this.paneView(side === 'a' ? 'b' : 'a')
    if (!other) return
    queueMicrotask(() => {
      const query = getSearchQuery(other.state)
      if (sameSearchOptions(query, options)) return
      other.dispatch({
        effects: setSearchQuery.of(
          new SearchQuery({
            search: query.search,
            replace: query.replace,
            caseSensitive: options.caseSensitive,
            regexp: options.regexp,
            wholeWord: query.wholeWord,
          }),
        ),
      })
    })
  }

  /** Lines added on the right and taken out on the left, summed over the chunks. */
  private diffStat(): DiffStat {
    if (!this.merge) return { added: 0, removed: 0 }
    const docA = this.merge.a.state.doc
    const docB = this.merge.b.state.doc
    let added = 0
    let removed = 0
    for (const chunk of this.merge.chunks) {
      removed += linesBetween(docA, chunk.fromA, chunk.toA)
      added += linesBetween(docB, chunk.fromB, chunk.toB)
    }
    return { added, removed }
  }

  private get views(): EditorView[] {
    if (this.single) return [this.single]
    if (this.merge) return [this.merge.a, this.merge.b]
    return []
  }

  private paneView(side: Side): EditorView | null {
    if (this.merge) return side === 'a' ? this.merge.a : this.merge.b
    return side === 'a' ? this.single : null
  }

  private get activeView(): EditorView {
    return this.paneView(this.activePane)!
  }

  // ---- the active pane ----------------------------------------------------

  focus(): void {
    this.activeView.focus()
  }

  /** False while the search panel or the preferences panel holds the keyboard. */
  get hasFocus(): boolean {
    return this.views.some((view) => view.hasFocus)
  }

  /**
   * Replaces the selection with `text`, the way a paste does.
   *
   * `input.paste` is the user event CodeMirror marks its own paste with, so the
   * history keeps the whole insertion as one step instead of joining it to the
   * typing around it.
   *
   * @param text what to put in
   */
  insertText(text: string): void {
    const view = this.activeView
    view.dispatch(view.state.replaceSelection(text), { scrollIntoView: true, userEvent: 'input.paste' })
  }

  // The view is deliberately not focused afterwards. CodeMirror mounts the
  // panel within the same update cycle and selects its find field there, so a
  // focus() here would take the keyboard straight back off it and the search
  // term would be typed into the draft. Nothing else depends on this call:
  // the caret Windows needs for the IME is the panel's own field, and the
  // window that comes back with nothing focused is handled in src/main.ts.
  //
  // The panel opens in the pane that has the keyboard, and in that one only.
  openSearch(): void {
    openSearchPanel(this.activeView)
  }

  undo(): void {
    if (!this.restoreClosedPane()) undoCommand(this.activeView)
    this.activeView.focus()
  }

  redo(): void {
    redoCommand(this.activeView)
    this.activeView.focus()
  }

  /** True while the history holds a step to undo. */
  get canUndo(): boolean {
    return undoDepth(this.activeView.state) > 0 || this.canRestoreClosedPane
  }

  /** True while the history holds an undone step to put back. */
  get canRedo(): boolean {
    return redoDepth(this.activeView.state) > 0
  }

  // ---- settings, which reach every pane ------------------------------------

  /** Whole lines only, or the changed characters within them as well. */
  setDiffMode(mode: DiffMode): void {
    this.diffMode = mode
    this.merge?.reconfigure({ highlightChanges: mode === 'char' })
  }

  async setLanguage(id: string): Promise<void> {
    this.reconfigure('language', await languageExtension(id))
  }

  setDark(dark: boolean): void {
    this.reconfigure('colors', colorExtension(dark))
  }

  setFont(size: number, family: string, weight: number): void {
    this.reconfigure('font', fontTheme(size, this.fontFamily(family), weight))
  }

  /** The tab width, and whether indenting puts in spaces or tab characters. */
  setTab(size: number, style: IndentStyle): void {
    this.reconfigure('tab', tabExtension(size, style))
  }

  /** Whether the spaces and tabs in the draft carry a mark. */
  setShowWhitespace(show: boolean): void {
    this.reconfigure('whitespace', whitespaceExtension(show))
  }

  /** Whether each level of indentation carries a rule. */
  setShowIndentGuides(show: boolean): void {
    this.reconfigure('indentGuides', indentGuideExtension(show))
  }

  /** Whether the draft carries a column of line numbers beside it. */
  setShowLineNumbers(show: boolean): void {
    this.reconfigure('lineNumbers', lineNumberExtension(show))
  }

  private reconfigure(slot: Slot, extension: Extension): void {
    this.current[slot] = extension
    const effect = this.compartments[slot].reconfigure(extension)
    for (const view of this.views) view.dispatch({ effects: effect })
  }

  private fontFamily(family: string): string {
    return family.trim() || this.defaultFontFamily
  }
}
