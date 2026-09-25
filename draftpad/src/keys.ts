// Key combinations, in the one form draftpad stores, compares and passes
// around: modifiers in a fixed order, then the key, joined with "+" —
// "Shift+Mod+V", "Alt+ArrowUp", "F11". "Mod" is Cmd on macOS and Ctrl
// elsewhere; "Ctrl" is the Control key on macOS and is never written on
// Windows, where Ctrl is always "Mod".
//
// A letter is written upper case, and Shift is always spelled out rather than
// read into the character: Shift+Mod+\ is "Shift+Mod+\", not "Mod+|". That is
// what lets a combination be matched the same way whatever the layout does to
// the shifted character, and what CodeMirror's own key names want too.

/** The order the modifiers are written in. */
const MODIFIERS = ['Ctrl', 'Alt', 'Shift', 'Mod'] as const
type Modifier = (typeof MODIFIERS)[number]

/** F1–F12, which is as far as the macOS menu bar's accelerators go. */
const FUNCTION_KEY = /^F([1-9]|1[0-2])$/

/** The keys that are not a character, by the name `KeyboardEvent.key` gives them. */
const NAMED_KEYS = new Set([
  'ArrowUp',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
  'Enter',
  'Backspace',
  'Delete',
  'Tab',
  'Escape',
  'Home',
  'End',
  'PageUp',
  'PageDown',
  'Space',
])

/**
 * The unshifted character on the key a `keyCode` names, for the keys whose
 * shifted character a layout may change. The same table CodeMirror falls back
 * on (w3c-keyname's `base`), so that what is recorded here is what its keymap
 * matches.
 */
const BASE: Record<number, string> = {
  186: ';',
  187: '=',
  188: ',',
  189: '-',
  190: '.',
  191: '/',
  192: '`',
  219: '[',
  220: '\\',
  221: ']',
  222: "'",
}
for (let code = 48; code <= 57; code++) BASE[code] = String.fromCharCode(code)
for (let code = 65; code <= 90; code++) BASE[code] = String.fromCharCode(code)

const PUNCTUATION = new Set(Object.values(BASE).filter((key) => !/^[0-9A-Z]$/.test(key)))

interface Parts {
  mods: Set<Modifier>
  key: string
}

function isKey(key: string): boolean {
  return /^[0-9A-Z]$/.test(key) || PUNCTUATION.has(key) || NAMED_KEYS.has(key) || FUNCTION_KEY.test(key)
}

/** Splits a combination, or returns null when it is not one draftpad can hold. */
function parse(combo: string): Parts | null {
  const parts = combo.split('+')
  const mods = new Set<Modifier>()
  // "Mod++" does not occur, but the key is whatever follows the last modifier.
  while (parts.length > 1 && (MODIFIERS as readonly string[]).includes(parts[0]!)) mods.add(parts.shift() as Modifier)
  const raw = parts.join('+')
  const key = raw.length === 1 ? raw.toUpperCase() : raw
  return isKey(key) ? { mods, key } : null
}

function join({ mods, key }: Parts): string {
  return [...MODIFIERS.filter((mod) => mods.has(mod)), key].join('+')
}

/** The same combination with its modifiers in order, or null when it is not a valid one. */
export function normalizeCombo(combo: string): string | null {
  const parts = parse(combo)
  return parts ? join(parts) : null
}

/**
 * Whether a combination may be given to a command: it has to hold something
 * other than Shift, or be a function key. A letter on its own, or with Shift,
 * is typing.
 */
export function isAssignable(combo: string): boolean {
  const parts = parse(combo)
  if (!parts) return false
  return FUNCTION_KEY.test(parts.key) || [...parts.mods].some((mod) => mod !== 'Shift')
}

/**
 * The combinations a key press could stand for, most literal first: the
 * character it produced, then the unshifted character on the key it was
 * pressed on. An Option or Shift on macOS turns L into ¬ or \ into |, and it
 * is the second one a binding is written against. Empty for a press of a
 * modifier on its own, and for anything draftpad cannot bind.
 *
 * AltGr on Windows arrives as Ctrl+Alt, and the character it produced is the
 * one being typed; the fallback is left out there, as CodeMirror leaves it.
 */
export function combosFromEvent(event: KeyboardEvent, platform: string): string[] {
  if (event.isComposing) return []
  const isMac = platform === 'macos'
  const mods = new Set<Modifier>()
  if (isMac ? event.metaKey : event.ctrlKey) mods.add('Mod')
  if (isMac && event.ctrlKey) mods.add('Ctrl')
  if (event.altKey) mods.add('Alt')
  if (event.shiftKey) mods.add('Shift')

  const found: string[] = []
  const add = (key: string | undefined): void => {
    if (!key || !isKey(key)) return
    const combo = join({ mods, key })
    if (!found.includes(combo)) found.push(combo)
  }
  const { key } = event
  if (key === ' ') add('Space')
  else if (key.length === 1) add(key.toUpperCase())
  else add(key)
  const altGr = !isMac && event.ctrlKey && event.altKey
  if (key.length === 1 && !altGr) add(BASE[event.keyCode])
  return found
}

/**
 * The combination a key press should be recorded as: the key it was pressed
 * on while a modifier other than Shift is held, which is what a binding is
 * matched against, and the character otherwise.
 */
export function recordFromEvent(event: KeyboardEvent, platform: string): string | null {
  const combos = combosFromEvent(event, platform)
  const held = event.ctrlKey || event.altKey || event.metaKey
  return (held ? combos.at(-1) : combos[0]) ?? null
}

const MAC_MODIFIERS: Record<Modifier, string> = { Ctrl: '⌃', Alt: '⌥', Shift: '⇧', Mod: '⌘' }
const MAC_KEYS: Record<string, string> = {
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
  Enter: '↩',
  Backspace: '⌫',
  Delete: '⌦',
  Tab: '⇥',
  Escape: '⎋',
  Home: '↖',
  End: '↘',
  PageUp: '⇞',
  PageDown: '⇟',
}
const OTHER_KEYS: Record<string, string> = {
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
  Escape: 'Esc',
}

/** How a combination is shown: ⇧⌘V on macOS, the order the menu bar uses; Ctrl+Shift+V elsewhere. */
export function formatCombo(combo: string, platform: string): string {
  const parts = parse(combo)
  if (!parts) return combo
  const { mods, key } = parts
  if (platform === 'macos') {
    return MODIFIERS.filter((mod) => mods.has(mod)).map((mod) => MAC_MODIFIERS[mod]).join('') + (MAC_KEYS[key] ?? key)
  }
  const names = (['Mod', 'Ctrl', 'Alt', 'Shift'] as const).filter((mod) => mods.has(mod)).map((mod) => (mod === 'Mod' ? 'Ctrl' : mod))
  return [...names, OTHER_KEYS[key] ?? key].join('+')
}

/** The same combination as a CodeMirror key name: "Shift-Mod-k", "Alt-ArrowUp". */
export function toCodeMirror(combo: string): string {
  const parts = parse(combo)
  if (!parts) throw new Error(`draftpad: not a key combination: ${combo}`)
  const key = /^[A-Z]$/.test(parts.key) ? parts.key.toLowerCase() : parts.key
  return [...MODIFIERS.filter((mod) => parts.mods.has(mod)), key].join('-')
}

const ACCELERATOR_KEYS: Record<string, string> = {
  ';': 'Semicolon',
  '=': 'Equal',
  ',': 'Comma',
  '-': 'Minus',
  '.': 'Period',
  '/': 'Slash',
  '`': 'Backquote',
  '[': 'BracketLeft',
  '\\': 'Backslash',
  ']': 'BracketRight',
  "'": 'Quote',
}

/**
 * The same combination as a menu accelerator, in the spelling muda parses
 * (src-tauri/src/menu.rs): "CmdOrCtrl+Shift+V". Punctuation is written by
 * name, since "+" is what the accelerator is split on.
 */
export function toAccelerator(combo: string): string {
  const parts = parse(combo)
  if (!parts) throw new Error(`draftpad: not a key combination: ${combo}`)
  const names: Record<Modifier, string> = { Ctrl: 'Ctrl', Alt: 'Alt', Shift: 'Shift', Mod: 'CmdOrCtrl' }
  // The order src-tauri/src/menu.rs writes its own in.
  const order = ['Mod', 'Ctrl', 'Alt', 'Shift'] as const
  return [...order.filter((mod) => parts.mods.has(mod)).map((mod) => names[mod]), ACCELERATOR_KEYS[parts.key] ?? parts.key].join('+')
}
