// The marks that make the spaces and tabs in the two panes visible.
//
// `highlightWhitespace` from the view package covers what an editor written for
// Latin text needs: its pattern is `/\t| /`, so the tab and the half-width
// space, each as a mark of its own that style.css paints. The full-width space
// (U+3000) is not in it, and a pad for comparing Japanese text cannot leave it
// out: a full-width space that draws nothing reads as no space at all, which is
// the one difference someone opened diffpad to find. The rule below marks it,
// and style.css gives it the same dot in its own wider box.
//
// Line breaks and the end of the document are deliberately unmarked. Once every
// space carries a dot, where a line ends is already plain, and a mark on every
// line is noise.

import type { Extension } from '@codemirror/state'
import {
  Decoration,
  type DecorationSet,
  type EditorView,
  highlightWhitespace,
  MatchDecorator,
  ViewPlugin,
  type ViewUpdate,
} from '@codemirror/view'

const ideographicSpace = new MatchDecorator({
  regexp: /　/g,
  decoration: Decoration.mark({ class: 'cm-ideographicSpace' }),
  // Nothing that is not whitespace can be part of a match, which is what lets
  // the decorator rescan only around an edit. `\s` covers U+3000.
  boundary: /\S/,
})

const highlightIdeographicSpace = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet

    constructor(view: EditorView) {
      this.decorations = ideographicSpace.createDeco(view)
    }

    update(update: ViewUpdate): void {
      this.decorations = ideographicSpace.updateDeco(update, this.decorations)
    }
  },
  { decorations: (plugin) => plugin.decorations },
)

/** Marks every tab, half-width space and full-width space in a pane. */
export const whitespaceMarks: Extension = [highlightWhitespace(), highlightIdeographicSpace]
