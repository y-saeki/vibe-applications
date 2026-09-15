// CodeMirror setup. Everything a setting can change lives in a Compartment so
// it can be swapped at runtime without rebuilding the editor.

import { autocompletion, closeBrackets, closeBracketsKeymap, completeAnyWord, completionKeymap } from '@codemirror/autocomplete'
import { defaultKeymap, history, historyKeymap, indentWithTab, redo as redoCommand, undo as undoCommand } from '@codemirror/commands'
import { bracketMatching, defaultHighlightStyle, indentOnInput, indentUnit, syntaxHighlighting } from '@codemirror/language'
import { getSearchQuery, highlightSelectionMatches, openSearchPanel, search, searchKeymap } from '@codemirror/search'
import { Compartment, EditorState, type Extension } from '@codemirror/state'
import { drawSelection, dropCursor, EditorView, keymap, type KeyBinding } from '@codemirror/view'

import { darkTheme } from './dark-theme'
import { languageExtension } from './languages'
import { searchPanelExtras } from './search-panel'
import type { State } from './state'
import { vimExtension } from './vim'

/** The two toggles in the search panel, as they are persisted. */
export interface SearchOptions {
  caseSensitive: boolean
  regexp: boolean
}

export interface EditorOptions {
  parent: HTMLElement
  initial: Readonly<State>
  /** Used when `state.fontFamily` is empty. */
  defaultFontFamily: string
  dark: boolean
  onDocChanged: () => void
  onSearchOptionsChanged: (options: SearchOptions) => void
}

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
  Completions: '入力候補',
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

function tabExtension(size: number): Extension {
  return [EditorState.tabSize.of(size), indentUnit.of(' '.repeat(size))]
}

function completionExtension(enabled: boolean): Extension {
  return enabled ? autocompletion({ override: [completeAnyWord], icons: false }) : []
}

function colorExtension(dark: boolean): Extension {
  return dark ? darkTheme : syntaxHighlighting(defaultHighlightStyle)
}

// The search extension reads these when it builds the initial query, which is
// the one the panel shows the first time it opens. Every later query inherits
// the flags from the one before it, so setting them here is enough to carry the
// toggles over from the previous run.
function searchExtension(initial: Readonly<State>): Extension {
  return search({ caseSensitive: initial.searchCaseSensitive, regexp: initial.searchRegexp })
}

function searchOptionsOf(state: EditorState): SearchOptions {
  const query = getSearchQuery(state)
  return { caseSensitive: query.caseSensitive, regexp: query.regexp }
}

export class Editor {
  readonly view: EditorView
  private readonly language = new Compartment()
  private readonly vim = new Compartment()
  private readonly colors = new Compartment()
  private readonly font = new Compartment()
  private readonly tab = new Compartment()
  private readonly completion = new Compartment()
  private readonly defaultFontFamily: string

  private constructor(options: EditorOptions, language: Extension, vim: Extension) {
    const { initial } = options
    this.defaultFontFamily = options.defaultFontFamily
    const state = EditorState.create({
      doc: initial.text,
      extensions: [
        // Vim must come before every other keymap.
        this.vim.of(vim),
        this.language.of(language),
        this.colors.of(colorExtension(options.dark)),
        this.font.of(fontTheme(initial.fontSize, this.fontFamily(initial.fontFamily), initial.fontWeight)),
        this.tab.of(tabExtension(initial.tabSize)),
        this.completion.of(completionExtension(initial.quickSuggestions)),
        history(),
        drawSelection(),
        dropCursor(),
        indentOnInput(),
        bracketMatching(),
        closeBrackets(),
        highlightSelectionMatches(),
        searchExtension(initial),
        searchPanelExtras(),
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({ spellcheck: 'false', autocorrect: 'off', autocapitalize: 'off' }),
        phrases,
        keymap.of([...closeBracketsKeymap, ...searchBindings, ...redoKeymap, ...historyKeymap, ...completionKeymap, ...defaultKeymap, indentWithTab]),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) options.onDocChanged()
          const before = searchOptionsOf(update.startState)
          const after = searchOptionsOf(update.state)
          if (after.caseSensitive !== before.caseSensitive || after.regexp !== before.regexp) {
            options.onSearchOptionsChanged(after)
          }
        }),
      ],
    })
    this.view = new EditorView({ state, parent: options.parent })
  }

  static async create(options: EditorOptions): Promise<Editor> {
    const [language, vim] = await Promise.all([
      languageExtension(options.initial.language),
      options.initial.editorMode === 'vim' ? vimExtension() : Promise.resolve<Extension>([]),
    ])
    return new Editor(options, language, vim)
  }

  getText(): string {
    return this.view.state.doc.toString()
  }

  get lineCount(): number {
    return this.view.state.doc.lines
  }

  focus(): void {
    this.view.focus()
  }

  openSearch(): void {
    openSearchPanel(this.view)
    this.view.focus()
  }

  undo(): void {
    undoCommand(this.view)
    this.view.focus()
  }

  redo(): void {
    redoCommand(this.view)
    this.view.focus()
  }

  async setLanguage(id: string): Promise<void> {
    const extension = await languageExtension(id)
    this.view.dispatch({ effects: this.language.reconfigure(extension) })
  }

  async setVim(enabled: boolean): Promise<void> {
    const extension = enabled ? await vimExtension() : []
    this.view.dispatch({ effects: this.vim.reconfigure(extension) })
  }

  setDark(dark: boolean): void {
    this.view.dispatch({ effects: this.colors.reconfigure(colorExtension(dark)) })
  }

  setFont(size: number, family: string, weight: number): void {
    this.view.dispatch({ effects: this.font.reconfigure(fontTheme(size, this.fontFamily(family), weight)) })
  }

  setTabSize(size: number): void {
    this.view.dispatch({ effects: this.tab.reconfigure(tabExtension(size)) })
  }

  setQuickSuggestions(enabled: boolean): void {
    this.view.dispatch({ effects: this.completion.reconfigure(completionExtension(enabled)) })
  }

  private fontFamily(family: string): string {
    return family.trim() || this.defaultFontFamily
  }
}
