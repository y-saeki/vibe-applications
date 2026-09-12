// The single command table. Menu items (macOS) and keyboard shortcuts
// (Windows) resolve to entries in this list. Ids match the menu item ids in
// src-tauri/src/menu.rs.

export interface Command {
  id: string
  title: string
  /** Shortcut in "Ctrl+Mod+F" form; "Mod" is Cmd on macOS and Ctrl elsewhere. */
  keys?: string
  run: () => void | Promise<void>
}

export interface CommandActions {
  openPreferences: () => void
  quit: () => Promise<void>
  toggleFullscreen: () => Promise<void>
  changeFontSize: (delta: number) => void
  openSearch: () => void
}

export function createCommands(actions: CommandActions, platform: string): Command[] {
  const isMac = platform === 'macos'
  const commands: Command[] = [
    { id: 'preferences', title: '環境設定…', keys: 'Mod+,', run: actions.openPreferences },
    { id: 'find', title: '検索・置換', keys: 'Mod+F', run: actions.openSearch },
    { id: 'increase_font_size', title: 'フォントを大きく', keys: 'Mod+=', run: () => actions.changeFontSize(1) },
    { id: 'decrease_font_size', title: 'フォントを小さく', keys: 'Mod+-', run: () => actions.changeFontSize(-1) },
    { id: 'toggle_fullscreen', title: 'フルスクリーンを切り替え', keys: isMac ? 'Ctrl+Mod+F' : 'F11', run: actions.toggleFullscreen },
    { id: 'close', title: '閉じる', keys: 'Mod+W', run: actions.quit },
    { id: 'quit', title: '終了', keys: 'Mod+Q', run: actions.quit },
  ]
  return commands
}

/**
 * Turns a keydown event into the "Ctrl+Mod+F" form used by `Command.keys`.
 * Returns null for plain typing so the caller can bail out fast.
 */
export function comboFromEvent(event: KeyboardEvent, platform: string): string | null {
  const isMac = platform === 'macos'
  const mod = isMac ? event.metaKey : event.ctrlKey
  const ctrl = isMac ? event.ctrlKey : false
  const key = normalizeKey(event)
  if (!key) return null
  const isFunctionKey = /^F\d{1,2}$/.test(key)
  if (!mod && !isFunctionKey) return null
  // '=' and '-' need Shift on some layouts (JIS: Shift+'-' types '='), so ignore it there.
  const shift = event.shiftKey && key !== '=' && key !== '-'
  const parts: string[] = []
  if (ctrl) parts.push('Ctrl')
  if (event.altKey) parts.push('Alt')
  if (shift) parts.push('Shift')
  if (mod) parts.push('Mod')
  parts.push(key)
  return parts.join('+')
}

function normalizeKey(event: KeyboardEvent): string | null {
  const { key, code } = event
  if (key === '+' || code === 'NumpadAdd') return '='
  if (code === 'NumpadSubtract') return '-'
  if (key.length === 1) return key.toUpperCase()
  if (/^F\d{1,2}$/.test(key)) return key
  return null
}

/**
 * Sorts commands so "Ctrl+Mod+F" style lookups are exact: the modifier order
 * produced by `comboFromEvent` is Ctrl, Alt, Shift, Mod.
 */
export function indexByKeys(commands: readonly Command[]): Map<string, Command> {
  const index = new Map<string, Command>()
  for (const command of commands) {
    if (!command.keys) continue
    const parts = command.keys.split('+')
    const key = parts.pop()!
    const order = ['Ctrl', 'Alt', 'Shift', 'Mod']
    const mods = order.filter((mod) => parts.includes(mod))
    index.set([...mods, key.length === 1 ? key.toUpperCase() : key].join('+'), command)
  }
  return index
}
