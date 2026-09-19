// The two panes and the diff between them, through CodeMirror's MergeView.
// Everything a setting can change lives in a Compartment so it can be swapped
// at runtime without rebuilding the editors. The merge view itself recomputes
// the chunks on every edit, on either side (line by line, see linediff.ts),
// and keeps the two panes aligned.

import { defaultKeymap, history, historyKeymap, indentWithTab, redo as redoCommand, undo as undoCommand } from '@codemirror/commands'
import { indentUnit } from '@codemirror/language'
import { getChunks, goToNextChunk, goToPreviousChunk, MergeView } from '@codemirror/merge'
import { Compartment, EditorState, type Extension, type StateEffect } from '@codemirror/state'
import { drawSelection, dropCursor, EditorView, keymap, lineNumbers, placeholder, type KeyBinding } from '@codemirror/view'

import { darkTheme } from './dark-theme'
import { installLineDiff } from './linediff'
import type { DiffMode, State, Texts } from './state'

/** The left pane is `a`, the right one `b`, as MergeView names them. */
export type Side = 'a' | 'b'

export interface EditorOptions {
  parent: HTMLElement
  initial: Readonly<State>
  /** Used when `state.fontFamily` is empty. */
  defaultFontFamily: string
  dark: boolean
  onDocChanged: () => void
  /** Called with how many changed chunks the two panes hold, whenever that is recomputed. */
  onChunksChanged: (count: number) => void
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

/** What an empty pane says about itself. */
const PLACEHOLDERS: Record<Side, string> = {
  a: '比較元のテキスト',
  b: '比較先のテキスト',
}

// historyKeymap binds redo to Mod-y everywhere and to Ctrl-Shift-z on Linux
// only, so Windows needs this one; on macOS it repeats the Cmd-Shift-z binding
// historyKeymap already has.
const redoKeymap: KeyBinding[] = [{ key: 'Mod-Shift-z', run: redoCommand, preventDefault: true }]

// The weight goes on the scroller with the family: CodeMirror's own theme
// leaves font-weight alone, so the content inherits it.
function fontTheme(size: number, family: string, weight: number): Extension {
  return EditorView.theme({
    '&': { fontSize: `${size}px` },
    '.cm-scroller': { fontFamily: family, fontWeight: String(weight) },
  })
}

function tabExtension(size: number): Extension {
  return [EditorState.tabSize.of(size), indentUnit.of(' '.repeat(size))]
}

function colorExtension(dark: boolean): Extension {
  return dark ? darkTheme : []
}

export class Editor {
  readonly merge: MergeView
  /** The pane that last had the keyboard, which is where a command acts. */
  private active: Side = 'a'
  private readonly colors = new Compartment()
  private readonly font = new Compartment()
  private readonly tab = new Compartment()
  private readonly defaultFontFamily: string

  constructor(options: EditorOptions) {
    const { initial } = options
    this.defaultFontFamily = options.defaultFontFamily
    installLineDiff()
    // One compartment serves both panes: each state keeps its own content for
    // it, and a change is dispatched to the two of them in turn.
    const shared: Extension = [
      this.colors.of(colorExtension(options.dark)),
      this.font.of(fontTheme(initial.fontSize, this.fontFamily(initial.fontFamily), initial.fontWeight)),
      this.tab.of(tabExtension(initial.tabSize)),
      history(),
      drawSelection(),
      dropCursor(),
      lineNumbers(),
      EditorView.lineWrapping,
      EditorView.contentAttributes.of({ spellcheck: 'false', autocorrect: 'off', autocapitalize: 'off' }),
      keymap.of([...redoKeymap, ...historyKeymap, ...defaultKeymap, indentWithTab]),
    ]
    const pane = (side: Side): Extension => [
      placeholder(PLACEHOLDERS[side]),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) options.onDocChanged()
        if (update.focusChanged && update.view.hasFocus) this.active = side
      }),
    ]
    // The merge view hands the recomputed chunks to both panes after an edit on
    // either, so listening on one of them is enough.
    const chunks = EditorView.updateListener.of((update) => {
      const before = getChunks(update.startState)?.chunks
      const after = getChunks(update.state)?.chunks
      if (before !== after) options.onChunksChanged(after?.length ?? 0)
    })
    this.merge = new MergeView({
      a: { doc: initial.textA, extensions: [shared, pane('a'), chunks] },
      b: { doc: initial.textB, extensions: [shared, pane('b')] },
      parent: options.parent,
      gutter: true,
      highlightChanges: initial.diffMode === 'char',
      diffConfig: { timeout: DIFF_TIMEOUT_MS },
    })
    options.onChunksChanged(this.merge.chunks.length)
  }

  private get activeView(): EditorView {
    return this.active === 'a' ? this.merge.a : this.merge.b
  }

  getTexts(): Texts {
    return {
      textA: this.merge.a.state.doc.toString(),
      textB: this.merge.b.state.doc.toString(),
    }
  }

  focus(): void {
    this.activeView.focus()
  }

  undo(): void {
    undoCommand(this.activeView)
    this.activeView.focus()
  }

  redo(): void {
    redoCommand(this.activeView)
    this.activeView.focus()
  }

  /** Moves the caret of the active pane to the next chunk, wrapping round at the end. */
  nextDiff(): void {
    goToNextChunk(this.activeView)
    this.activeView.focus()
  }

  previousDiff(): void {
    goToPreviousChunk(this.activeView)
    this.activeView.focus()
  }

  /** Whole lines only, or the changed characters within them as well. */
  setDiffMode(mode: DiffMode): void {
    this.merge.reconfigure({ highlightChanges: mode === 'char' })
  }

  setDark(dark: boolean): void {
    this.reconfigureBoth(this.colors.reconfigure(colorExtension(dark)))
  }

  setFont(size: number, family: string, weight: number): void {
    this.reconfigureBoth(this.font.reconfigure(fontTheme(size, this.fontFamily(family), weight)))
  }

  setTabSize(size: number): void {
    this.reconfigureBoth(this.tab.reconfigure(tabExtension(size)))
  }

  private reconfigureBoth(effect: StateEffect<unknown>): void {
    for (const view of [this.merge.a, this.merge.b]) view.dispatch({ effects: effect })
  }

  private fontFamily(family: string): string {
    return family.trim() || this.defaultFontFamily
  }
}
