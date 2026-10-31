// Where a wrapped line breaks around a long unbroken run such as a URL.
//
// `EditorView.lineWrapping` lets the browser break anywhere through
// `overflow-wrap: anywhere`, but only as a last resort: a run with no break
// opportunity of its own goes to the next row whole when there is a space in
// front of it, and is broken only there. A line like "1: https://…" therefore
// leaves "1:" alone on its first row, with most of that row empty.
//
// The runs matched below are given `word-break: break-all` by style.css, which
// opens a break opportunity between every two of their characters, so the
// browser fills the row before breaking. Ordinary words are left alone and
// still wrap whole: the pattern only takes runs longer than any word.
//
// The run is printable ASCII without the space, which is what URLs, paths and
// hashes are made of. Japanese breaks between characters already, and a
// pattern that took any non-space would swallow a whole Japanese sentence,
// with the English words inside it broken along with it.

import {
  Decoration,
  type DecorationSet,
  type EditorView,
  MatchDecorator,
  ViewPlugin,
  type ViewUpdate,
} from '@codemirror/view'

/** How many characters a run takes before it is broken anywhere. */
export const LONG_WORD_LENGTH = 30

const longWord = new MatchDecorator({
  regexp: new RegExp(`[!-~]{${LONG_WORD_LENGTH},}`, 'g'),
  decoration: Decoration.mark({ class: 'cm-longWord' }),
  // Nothing outside printable ASCII can be part of a match, so an edit
  // rescans the run it lands in and no further.
  boundary: /[^!-~]/,
})

/** Lets the long runs in the draft break at the end of a row. */
export const longWords = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet

    constructor(view: EditorView) {
      this.decorations = longWord.createDeco(view)
    }

    update(update: ViewUpdate): void {
      this.decorations = longWord.updateDeco(update, this.decorations)
    }
  },
  { decorations: (plugin) => plugin.decorations },
)
