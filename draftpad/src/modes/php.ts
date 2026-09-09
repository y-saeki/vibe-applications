// PHP highlighting built on the generic C-like stream parser from
// @codemirror/legacy-modes (which ships no PHP mode of its own). Handles
// `$variables`, `#` comments and `<?php ... ?>` tags on top of the C-like base.

import type { StringStream } from '@codemirror/language'
import { clike } from '@codemirror/legacy-modes/mode/clike'

function words(list: string): Record<string, boolean> {
  const out: Record<string, boolean> = {}
  for (const word of list.split(/\s+/)) if (word) out[word] = true
  return out
}

export const php = clike({
  name: 'php',
  keywords: words(`
    abstract and array as break callable case catch class clone const continue declare default
    do echo else elseif empty enddeclare endfor endforeach endif endswitch endwhile enum eval
    exit extends final finally fn for foreach function global goto if implements include
    include_once instanceof insteadof interface isset list match namespace new or print private
    protected public readonly require require_once return static switch throw trait try unset
    use var while xor yield
  `),
  types: words('int float bool string array object mixed void null never iterable self parent'),
  atoms: words('true false null TRUE FALSE NULL'),
  hooks: {
    // The hook is called after the trigger character has been consumed.
    $: (stream: StringStream) => {
      stream.eatWhile(/\w/)
      return 'variableName'
    },
    '#': (stream: StringStream) => {
      stream.skipToEnd()
      return 'comment'
    },
    '<': (stream: StringStream) => (stream.match(/^\?(?:php\b|=)?/i) ? 'meta' : false),
    '?': (stream: StringStream) => (stream.eat('>') ? 'meta' : false),
  },
})
