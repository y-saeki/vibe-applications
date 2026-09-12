// The single command table. Menu items (macOS), keyboard shortcuts (Windows)
// and the command palette all resolve to entries in this list. Ids match the
// menu item ids in src-tauri/src/menu.rs.

import { LANGUAGES } from './languages'
import type { Theme } from './state'

export interface Command {
  id: string
  title: string
  /** Shortcut in "Mod+Shift+P" form; "Mod" is Cmd on macOS and Ctrl elsewhere. */
  keys?: string
  /**
   * Kept out of the command palette. For commands that only make sense from
   * the menu or a shortcut, such as opening the palette itself.
   */
  hiddenInPalette?: boolean
  run: () => void | Promise<void>
}

export interface CommandActions {
  openPreferences: () => void
  openPalette: () => void
  quit: () => Promise<void>
  toggleFullscreen: () => Promise<void>
  changeFontSize: (delta: number) => void
  toggleVim: () => void
  toggleAlwaysOnTop: () => void
  setTheme: (theme: Theme) => void
  setLanguage: (id: string) => void
  openSearch: () => void
}

export function createCommands(actions: CommandActions, platform: string): Command[] {
  const isMac = platform === 'macos'
  const commands: Command[] = [
    { id: 'preferences', title: '環境設定…', keys: 'Mod+,', run: actions.openPreferences },
    { id: 'command_palette', title: 'コマンドパレット…', keys: 'Mod+Shift+P', hiddenInPalette: true, run: actions.openPalette },
    { id: 'find', title: '検索・置換', keys: 'Mod+F', run: actions.openSearch },
    { id: 'increase_font_size', title: 'フォントを大きく', keys: 'Mod+=', run: () => actions.changeFontSize(1) },
    { id: 'decrease_font_size', title: 'フォントを小さく', keys: 'Mod+-', run: () => actions.changeFontSize(-1) },
    { id: 'toggle_vim', title: 'Vim モードを切り替え', run: actions.toggleVim },
    { id: 'toggle_always_on_top', title: '常に手前に表示を切り替え', run: actions.toggleAlwaysOnTop },
    { id: 'toggle_fullscreen', title: 'フルスクリーンを切り替え', keys: isMac ? 'Ctrl+Mod+F' : 'F11', run: actions.toggleFullscreen },
    { id: 'theme:system', title: 'テーマ: システムに合わせる', run: () => actions.setTheme('system') },
    { id: 'theme:light', title: 'テーマ: ライト', run: () => actions.setTheme('light') },
    { id: 'theme:dark', title: 'テーマ: ダーク', run: () => actions.setTheme('dark') },
    ...LANGUAGES.map((lang) => ({
      id: `language:${lang.id}`,
      title: `言語: ${lang.label}`,
      run: () => actions.setLanguage(lang.id),
    })),
    { id: 'close', title: '閉じる', keys: 'Mod+W', run: actions.quit },
    { id: 'quit', title: '終了', keys: 'Mod+Q', run: actions.quit },
  ]
  return commands
}

/** Human-readable shortcut label for the palette. */
export function formatKeys(keys: string, platform: string): string {
  const isMac = platform === 'macos'
  const parts = keys.split('+').map((part) => {
    switch (part) {
      case 'Mod':
        return isMac ? '⌘' : 'Ctrl'
      case 'Shift':
        return isMac ? '⇧' : 'Shift'
      case 'Alt':
        return isMac ? '⌥' : 'Alt'
      case 'Ctrl':
        return isMac ? '⌃' : 'Ctrl'
      default:
        return part
    }
  })
  return isMac ? parts.join('') : parts.join('+')
}

/**
 * Turns a keydown event into the "Mod+Shift+P" form used by `Command.keys`.
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
 * Sorts commands so "Mod+Shift+P" style lookups are exact: the modifier order
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
