// CodeMirror setup. Everything a setting can change lives in a Compartment so
// it can be swapped at runtime without rebuilding the editor.

import { autocompletion, closeBrackets, closeBracketsKeymap, completeAnyWord, completionKeymap } from '@codemirror/autocomplete'
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import { bracketMatching, defaultHighlightStyle, indentOnInput, indentUnit, syntaxHighlighting } from '@codemirror/language'
import { highlightSelectionMatches, openSearchPanel, search, searchKeymap } from '@codemirror/search'
import { Compartment, EditorState, type Extension } from '@codemirror/state'
import { oneDark } from '@codemirror/theme-one-dark'
import { drawSelection, dropCursor, EditorView, keymap } from '@codemirror/view'

import { languageExtension } from './languages'
import type { State } from './state'
import { vimExtension } from './vim'

export interface EditorOptions {
  parent: HTMLElement
  initial: Readonly<State>
  /** Used when `state.fontFamily` is empty. */
  defaultFontFamily: string
  dark: boolean
  onDocChanged: () => void
}

const phrases = EditorState.phrases.of({
  Find: '検索',
  Replace: '置換',
  next: '次へ',
  previous: '前へ',
  all: 'すべて選択',
  'match case': '大文字小文字を区別',
  'by word': '単語単位',
  regexp: '正規表現',
  replace: '置換',
  'replace all': 'すべて置換',
  close: '閉じる',
  'current match': '現在の一致',
  'replaced $ matches': '$ 件を置換しました',
  'replaced match on line $': '$ 行目の一致を置換しました',
  'on line': '行',
  'Go to line': '行へ移動',
  go: '移動',
  Completions: '入力候補',
})

function fontTheme(size: number, family: string): Extension {
  return EditorView.theme({
    '&': { fontSize: `${size}px` },
    '.cm-scroller': { fontFamily: family },
  })
}

function tabExtension(size: number): Extension {
  return [EditorState.tabSize.of(size), indentUnit.of(' '.repeat(size))]
}

function completionExtension(enabled: boolean): Extension {
  return enabled ? autocompletion({ override: [completeAnyWord], icons: false }) : []
}

function colorExtension(dark: boolean): Extension {
  return dark ? oneDark : syntaxHighlighting(defaultHighlightStyle)
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
        this.font.of(fontTheme(initial.fontSize, this.fontFamily(initial.fontFamily))),
        this.tab.of(tabExtension(initial.tabSize)),
        this.completion.of(completionExtension(initial.quickSuggestions)),
        history(),
        drawSelection(),
        dropCursor(),
        indentOnInput(),
        bracketMatching(),
        closeBrackets(),
        highlightSelectionMatches(),
        search(),
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({ spellcheck: 'false', autocorrect: 'off', autocapitalize: 'off' }),
        phrases,
        keymap.of([...closeBracketsKeymap, ...searchKeymap, ...historyKeymap, ...completionKeymap, ...defaultKeymap, indentWithTab]),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) options.onDocChanged()
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

  setFont(size: number, family: string): void {
    this.view.dispatch({ effects: this.font.reconfigure(fontTheme(size, this.fontFamily(family))) })
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
