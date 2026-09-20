// The faint rules that show how deep in an indented block a line sits.
//
// CodeMirror has nothing of the sort, and the shape of a guide decides how it
// has to be drawn. A rule has to run the whole height of the line and meet the
// one on the line below it: anything hung on the indentation characters
// themselves — a border on a mark decoration, say — is an inline box as tall as
// the text rather than as tall as the line, so a column of them comes out
// dashed. A blank line inside a block has no character to hang one on at all.
//
// So a guide is a background on the line instead. This marks every line that is
// in a block with the number of levels it is in, and style.css draws that many
// rules across the line's own box, which has no gap above or below it.
//
// The width of a level is the tab width setting, the same number CodeMirror
// lays its tabs out from, so the rules stand where the indentation steps.

import { countColumn, EditorState, type Extension, type Line, RangeSetBuilder, type Text } from '@codemirror/state'
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from '@codemirror/view'

/**
 * How many whole levels of indentation a line carries, or null when the line
 * holds nothing but whitespace.
 *
 * Only spaces and tabs count. A full-width space is a character the draft is
 * written in rather than a step of a structure, and it is no multiple of the
 * tab width wide, so a rule drawn after one would stand between two columns.
 *
 * @param text the line
 * @param tabSize how many columns a tab covers, which is also how wide a level
 *   is: indentation that does not fill a level does not open one
 */
function levelsOf(text: string, tabSize: number): number | null {
  let end = 0
  while (end < text.length && (text[end] === ' ' || text[end] === '\t')) end++
  if (end === text.length) return null
  return Math.floor(countColumn(text, tabSize, end) / tabSize)
}

/** The nearest line above `number` that holds something, as levels. */
function levelsAbove(doc: Text, number: number, tabSize: number): number {
  for (let n = number - 1; n >= 1; n--) {
    const levels = levelsOf(doc.line(n).text, tabSize)
    if (levels !== null) return levels
  }
  return 0
}

/** The nearest line below `number` that holds something, as levels. */
function levelsBelow(doc: Text, number: number, tabSize: number): number {
  for (let n = number + 1; n <= doc.lines; n++) {
    const levels = levelsOf(doc.line(n).text, tabSize)
    if (levels !== null) return levels
  }
  return 0
}

// One decoration per depth, kept rather than rebuilt: a rebuild that lands on
// the same number for a line is then the same object, and CodeMirror leaves
// that line's DOM alone.
const byLevels = new Map<number, Decoration>()

function guide(levels: number): Decoration {
  let decoration = byLevels.get(levels)
  if (!decoration) {
    decoration = Decoration.line({
      class: 'cm-indentGuides',
      attributes: { style: `--indent-guide-levels: ${levels}` },
    })
    byLevels.set(levels, decoration)
  }
  return decoration
}

function buildGuides(view: EditorView): DecorationSet {
  const { doc, tabSize } = view.state
  const builder = new RangeSetBuilder<Decoration>()
  const add = (line: Line, levels: number): void => {
    if (levels > 0) builder.add(line.from, line.from, guide(levels))
  }

  for (const range of view.visibleRanges) {
    const first = doc.lineAt(range.from).number
    const last = doc.lineAt(range.to).number
    // A blank line inside a block carries the guides of the block, so it takes
    // its count from the two lines around it — the shallower of them, which is
    // what stops a block that has ended from trailing a rule down the empty
    // lines under it. The line above may be outside the range, whenever the
    // viewport opens in the middle of a block.
    let above = levelsAbove(doc, first, tabSize)
    let blanks: Line[] = []

    for (let number = first; number <= last; number++) {
      const line = doc.line(number)
      const levels = levelsOf(line.text, tabSize)
      if (levels === null) {
        blanks.push(line)
        continue
      }
      for (const blank of blanks) add(blank, Math.min(above, levels))
      blanks = []
      above = levels
      add(line, levels)
    }

    // A run that reaches the bottom of the viewport: the line that closes it is
    // below, and looking for it once is enough for the whole run.
    if (blanks.length > 0) {
      const below = levelsBelow(doc, last, tabSize)
      for (const blank of blanks) add(blank, Math.min(above, below))
    }
  }
  return builder.finish()
}

const guidePlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet

    constructor(view: EditorView) {
      this.decorations = buildGuides(view)
    }

    update(update: ViewUpdate): void {
      const tabSizeChanged = update.startState.tabSize !== update.state.tabSize
      if (update.docChanged || update.viewportChanged || tabSizeChanged) {
        this.decorations = buildGuides(update.view)
      }
    }
  },
  { decorations: (plugin) => plugin.decorations },
)

// How wide one level is, for style.css, in columns rather than pixels: the
// stylesheet turns it into a length against the editor's own font, the way
// CodeMirror's `tab-size` on the same element does. It is recomputed from the
// tab width setting, so changing that moves the rules with the tabs.
const levelWidth = EditorView.contentAttributes.compute([EditorState.tabSize], (state) => ({
  style: `--indent-guide-step: ${state.tabSize}`,
}))

/** Rules every line that is inside an indented block, one per level. */
export const indentGuides: Extension = [guidePlugin, levelWidth]
