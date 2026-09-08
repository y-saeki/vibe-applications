// Solidity highlighting built on the generic C-like stream parser from
// @codemirror/legacy-modes, so no extra dependency is needed.

import { clike } from '@codemirror/legacy-modes/mode/clike'

function words(list: string): Record<string, boolean> {
  const out: Record<string, boolean> = {}
  for (const word of list.split(/\s+/)) if (word) out[word] = true
  return out
}

function sized(prefix: string, step: number, max: number): string {
  const out: string[] = []
  for (let n = step; n <= max; n += step) out.push(`${prefix}${n}`)
  return out.join(' ')
}

export const solidity = clike({
  name: 'solidity',
  keywords: words(`
    pragma solidity abicoder contract interface library abstract is function modifier
    constructor fallback receive event emit struct enum mapping using for if else while do
    break continue return returns revert require assert new delete this super memory storage
    calldata public private internal external pure view payable constant immutable virtual
    override indexed anonymous unchecked try catch import as from assembly type error let
    unicode hex
  `),
  types: words(`
    address bool string bytes byte var fixed ufixed
    ${sized('int', 8, 256)} ${sized('uint', 8, 256)} ${sized('bytes', 1, 32)}
  `),
  atoms: words('true false wei gwei ether seconds minutes hours days weeks msg block tx now'),
  builtin: words('keccak256 sha256 ripemd160 ecrecover addmod mulmod selfdestruct blockhash gasleft abi'),
})
