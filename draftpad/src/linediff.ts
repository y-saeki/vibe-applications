// Line-level diffing for the compare pane (src/editor.ts builds it on a MergeView).
//
// @codemirror/merge diffs the two texts character by character and then
// groups the changed characters into chunks along line boundaries. Two things
// follow from that which a side-by-side view should not do. A line added at
// the end of a text is, character-wise, a line break appended to the previous
// line, so the previous line is painted as changed on both sides and no gap
// opens opposite the new one. And a deletion and an insertion with a short
// unchanged line between them are merged into one block, unchanged line
// included. VS Code and difff pair the lines up first and only then look inside
// them, which is what this does.
//
// The merge view has no option for it, but it fetches its chunks through
// Chunk.build / Chunk.updateA / Chunk.updateB, three static methods of an
// exported class, at the time it needs them. `installLineDiff` replaces those
// with a line-level computation; everything downstream of the chunks (the
// marks, the spacers, the navigation) is unchanged. The originals are kept for
// the one input this cannot encode, see `encode`.

import { Change, Chunk, type DiffConfig, diff, presentableDiff } from '@codemirror/merge'
import type { Text } from '@codemirror/state'

const original = {
  build: Chunk.build,
  updateA: Chunk.updateA,
  updateB: Chunk.updateB,
}

let installed = false

/**
 * Makes every merge view compute its chunks line by line. Idempotent.
 *
 * The merge view diffs incrementally, around the edit; this recomputes the
 * whole diff on every edit instead. The line-level pass works on one code unit
 * per line and the character-level passes only on the changed blocks, so this
 * is cheap for two similar texts of any realistic size.
 */
export function installLineDiff(): void {
  if (installed) return
  installed = true
  Chunk.build = (a, b, conf) => buildLineChunks(a, b, conf) ?? original.build(a, b, conf)
  Chunk.updateA = (_chunks, a, b, _changes, conf) => Chunk.build(a, b, conf)
  Chunk.updateB = (_chunks, a, b, _changes, conf) => Chunk.build(a, b, conf)
}

/**
 * The chunks between `a` and `b` with lines paired up first, or null when the
 * texts hold more distinct lines than `encode` can tell apart.
 */
export function buildLineChunks(a: Text, b: Text, conf?: DiffConfig): readonly Chunk[] | null {
  const ids = new Map<string, number>()
  const encodedA = encode(a, ids)
  const encodedB = encodedA === null ? null : encode(b, ids)
  if (encodedA === null || encodedB === null) return null

  // One time budget for the whole computation, not one per pass.
  const deadline = conf?.timeout === undefined ? undefined : Date.now() + conf.timeout
  const budget = (): DiffConfig => (deadline === undefined ? { ...conf } : { ...conf, timeout: Math.max(1, deadline - Date.now()) })

  return diff(encodedA, encodedB, budget()).map((lines) => {
    // `lines` is in line indices; the chunk wants document positions, with
    // `to` one past the end of the last line, as the merge view builds them.
    const fromA = lineStart(a, lines.fromA)
    const toA = lines.toA > lines.fromA ? a.line(lines.toA).to + 1 : fromA
    const fromB = lineStart(b, lines.fromB)
    const toB = lines.toB > lines.fromB ? b.line(lines.toB).to + 1 : fromB
    // What differs within the block, for the character marks; relative to
    // the block's start, as the merge view stores them.
    const textA = a.sliceString(fromA, Math.max(fromA, toA - 1))
    const textB = b.sliceString(fromB, Math.max(fromB, toB - 1))
    const changes = textA && textB ? presentableDiff(textA, textB, budget()) : [new Change(0, textA.length, 0, textB.length)]
    return new Chunk(changes, fromA, toA, fromB, toB)
  })
}

/**
 * Where the line at index `line` (0-based) starts; the end of the document
 * when `line` is one past the last line, which is where an insertion after
 * the last line sits.
 */
function lineStart(doc: Text, line: number): number {
  return line < doc.lines ? doc.line(line + 1).from : doc.length
}

/**
 * The document as one UTF-16 code unit per line, equal lines to equal units,
 * so that the merge view's character diff of two such strings is a line diff
 * of the documents. `ids` numbers the distinct lines across both documents.
 *
 * The units skip the surrogate range, which the diff would otherwise split,
 * leaving room for 63,488 distinct lines; a pair of texts with more returns
 * null, and the caller falls back to the character diff.
 */
function encode(doc: Text, ids: Map<string, number>): string | null {
  let out = ''
  for (const iter = doc.iterLines(); !iter.next().done; ) {
    let id = ids.get(iter.value)
    if (id === undefined) {
      id = ids.size
      if (id >= CAPACITY) return null
      ids.set(iter.value, id)
    }
    out += String.fromCharCode(id < SURROGATE_FROM ? id : id + SURROGATE_LENGTH)
  }
  return out
}

const SURROGATE_FROM = 0xd800
const SURROGATE_LENGTH = 0x800
const CAPACITY = 0x10000 - SURROGATE_LENGTH
