// Every keyboard shortcut draftpad has, and which key each one is on.
//
// Two kinds. The ones in SHORTCUTS can be given another key, or none, in the
// preferences panel; the rest are listed in FIXED, shown there and never
// changed: the clipboard, undo, moving the caret, typing itself. Those are
// the keys every text field on the system answers the same way, several of
// them are items AppKit owns on macOS, and a draft that no longer answers
// Backspace is not one anybody can fix from inside it.
//
// Which command a shortcut runs is not decided here. The application's own
// commands are `src/commands.ts`; the editor's are CodeMirror's, and
// `src/editor-keymap.ts` binds them. This is only the catalogue and the keys,
// so that the panel, the menu bar and both keymaps read one list.

import { isAssignable, normalizeCombo } from './keys'

/** The group a shortcut is listed under, in the order the panel lists them. */
export const GROUPS = ['アプリ', '編集', '選択・移動', '検索', 'その他'] as const
export type Group = (typeof GROUPS)[number]

/**
 * The keys a shortcut starts on, per platform. `null` on a platform means the
 * shortcut is not a changeable one there: macOS's full screen item is one
 * AppKit draws, and it keeps its own key.
 */
interface Defaults {
  mac: readonly string[] | null
  other: readonly string[] | null
}

export interface Shortcut {
  /** Matches the command's id in `src/commands.ts`, or the editor command's in `src/editor-keymap.ts`. */
  id: string
  title: string
  group: Group
  keys: Defaults
}

/** Same keys on both platforms. */
function both(...keys: string[]): Defaults {
  return { mac: keys, other: keys }
}

export const SHORTCUTS: readonly Shortcut[] = [
  { id: 'preferences', title: '環境設定…', group: 'アプリ', keys: both('Mod+,') },
  { id: 'open_compare', title: 'テキストを比較する', group: 'アプリ', keys: both('Mod+\\') },
  { id: 'close', title: '閉じる', group: 'アプリ', keys: both('Mod+W') },
  { id: 'quit', title: '終了', group: 'アプリ', keys: both('Mod+Q') },
  { id: 'toggle_fullscreen', title: 'フルスクリーンを切り替え', group: 'アプリ', keys: { mac: null, other: ['F11'] } },

  { id: 'paste_plain', title: 'プレーンテキストとして貼り付け', group: '編集', keys: both('Shift+Mod+V') },
  { id: 'move_line_up', title: '行を上へ移動', group: '編集', keys: both('Alt+ArrowUp') },
  { id: 'move_line_down', title: '行を下へ移動', group: '編集', keys: both('Alt+ArrowDown') },
  { id: 'copy_line_up', title: '行を上に複製', group: '編集', keys: both('Alt+Shift+ArrowUp') },
  { id: 'copy_line_down', title: '行を下に複製', group: '編集', keys: both('Alt+Shift+ArrowDown') },
  { id: 'delete_line', title: '行を削除', group: '編集', keys: both('Shift+Mod+K') },
  { id: 'insert_blank_line', title: '下に空行を挿入', group: '編集', keys: both('Mod+Enter') },
  { id: 'indent_more', title: 'インデントを増やす', group: '編集', keys: both('Mod+]') },
  { id: 'indent_less', title: 'インデントを減らす', group: '編集', keys: both('Mod+[') },
  { id: 'indent_selection', title: 'インデントを整える', group: '編集', keys: both('Alt+Mod+\\') },
  { id: 'toggle_comment', title: 'コメントを切り替え', group: '編集', keys: both('Mod+/') },
  { id: 'toggle_block_comment', title: 'ブロックコメントを切り替え', group: '編集', keys: { mac: ['Ctrl+Shift+A'], other: ['Alt+Shift+A'] } },

  { id: 'select_line', title: '行を選択', group: '選択・移動', keys: { mac: ['Ctrl+L'], other: ['Alt+L'] } },
  { id: 'select_next', title: '単語を選択', group: '選択・移動', keys: both('Mod+D') },
  { id: 'select_parent', title: '構文単位で選択を広げる', group: '選択・移動', keys: both('Mod+I') },
  { id: 'undo_selection', title: '選択を元に戻す', group: '選択・移動', keys: both('Mod+U') },
  { id: 'redo_selection', title: '選択をやり直す', group: '選択・移動', keys: { mac: ['Shift+Mod+U'], other: ['Alt+U'] } },
  { id: 'syntax_left', title: '構文単位で左へ移動', group: '選択・移動', keys: { mac: ['Ctrl+ArrowLeft'], other: ['Alt+ArrowLeft'] } },
  { id: 'syntax_right', title: '構文単位で右へ移動', group: '選択・移動', keys: { mac: ['Ctrl+ArrowRight'], other: ['Alt+ArrowRight'] } },
  { id: 'matching_bracket', title: '対応する括弧へ移動', group: '選択・移動', keys: both('Shift+Mod+\\') },

  { id: 'find', title: '検索・置換', group: '検索', keys: both('Mod+F') },
  { id: 'find_next', title: '次を検索', group: '検索', keys: both('Mod+G', 'F3') },
  { id: 'find_previous', title: '前を検索', group: '検索', keys: both('Shift+Mod+G', 'Shift+F3') },
  { id: 'goto_line', title: '行へ移動', group: '検索', keys: both('Alt+Mod+G') },

  { id: 'tab_focus_mode', title: 'Tab キーでフォーカスを移動', group: 'その他', keys: { mac: ['Alt+Shift+M'], other: ['Mod+M'] } },
]

/** A shortcut that is only shown. */
export interface FixedShortcut {
  title: string
  keys: Defaults
}

export interface FixedGroup {
  title: string
  /** A line under the heading, for what applies to the whole group. */
  note?: string
  items: readonly FixedShortcut[]
}

function macOnly(...keys: string[]): Defaults {
  return { mac: keys, other: null }
}

function otherOnly(...keys: string[]): Defaults {
  return { mac: null, other: keys }
}

export const FIXED: readonly FixedGroup[] = [
  {
    title: '基本の編集',
    items: [
      { title: '元に戻す', keys: both('Mod+Z') },
      { title: 'やり直す', keys: { mac: ['Shift+Mod+Z'], other: ['Mod+Y', 'Shift+Mod+Z'] } },
      { title: '切り取り', keys: both('Mod+X') },
      { title: 'コピー', keys: both('Mod+C') },
      { title: '貼り付け', keys: both('Mod+V') },
      { title: 'すべてを選択', keys: both('Mod+A') },
      { title: '改行', keys: both('Enter') },
      { title: '1 文字削除', keys: both('Backspace', 'Delete') },
      { title: '単語単位で削除', keys: { mac: ['Alt+Backspace', 'Alt+Delete'], other: ['Mod+Backspace', 'Mod+Delete'] } },
      { title: '行頭・行末まで削除', keys: macOnly('Mod+Backspace', 'Mod+Delete') },
      { title: 'インデント・インデント解除', keys: both('Tab', 'Shift+Tab') },
      { title: '選択を解除・検索を閉じる', keys: both('Escape') },
    ],
  },
  {
    title: 'カーソル移動',
    note: 'Shift を加えると、選択しながら移動します。',
    items: [
      { title: '1 文字', keys: both('ArrowLeft', 'ArrowRight') },
      { title: '1 行', keys: both('ArrowUp', 'ArrowDown') },
      { title: '単語単位', keys: { mac: ['Alt+ArrowLeft', 'Alt+ArrowRight'], other: ['Mod+ArrowLeft', 'Mod+ArrowRight'] } },
      { title: '行頭・行末', keys: { mac: ['Mod+ArrowLeft', 'Mod+ArrowRight', 'Home', 'End'], other: ['Home', 'End'] } },
      {
        title: '文書の先頭・末尾',
        keys: { mac: ['Mod+ArrowUp', 'Mod+ArrowDown', 'Mod+Home', 'Mod+End'], other: ['Mod+Home', 'Mod+End'] },
      },
      { title: '1 ページ', keys: { mac: ['PageUp', 'PageDown', 'Ctrl+ArrowUp', 'Ctrl+ArrowDown'], other: ['PageUp', 'PageDown'] } },
    ],
  },
  {
    title: 'macOS のテキスト操作',
    items: [
      { title: '行頭・行末へ移動', keys: macOnly('Ctrl+A', 'Ctrl+E') },
      { title: '1 文字戻る・進む', keys: macOnly('Ctrl+B', 'Ctrl+F') },
      { title: '前の行・次の行へ移動', keys: macOnly('Ctrl+P', 'Ctrl+N') },
      { title: '次のページへ移動', keys: macOnly('Ctrl+V') },
      { title: '前方・後方の 1 文字を削除', keys: macOnly('Ctrl+D', 'Ctrl+H') },
      { title: '単語単位で後方を削除', keys: macOnly('Ctrl+Alt+H') },
      { title: '行末まで削除', keys: macOnly('Ctrl+K') },
      { title: '行を分割', keys: macOnly('Ctrl+O') },
      { title: '前後の文字を入れ替え', keys: macOnly('Ctrl+T') },
    ],
  },
  {
    title: 'ウィンドウ',
    items: [
      { title: 'フルスクリーンを切り替え', keys: macOnly('Ctrl+Mod+F') },
      { title: 'draftpad を隠す', keys: macOnly('Mod+H') },
      { title: 'ほかを隠す', keys: macOnly('Alt+Mod+H') },
      { title: 'ウィンドウを閉じる', keys: otherOnly('Alt+F4') },
    ],
  },
]

/** The `keys` of a shortcut on one platform. */
export function keysOn(keys: Defaults, platform: string): readonly string[] | null {
  return platform === 'macos' ? keys.mac : keys.other
}

/** The changeable shortcuts on one platform, in the panel's order. */
export function shortcutsOn(platform: string): Shortcut[] {
  return SHORTCUTS.filter((shortcut) => keysOn(shortcut.keys, platform) !== null)
}

/** The fixed groups on one platform, each holding only what that platform has. */
export function fixedOn(platform: string): FixedGroup[] {
  return FIXED.map((group) => ({ ...group, items: group.items.filter((item) => keysOn(item.keys, platform)?.length) })).filter(
    (group) => group.items.length > 0,
  )
}

/** Which fixed shortcut a combination belongs to on this platform, by its title, or null. */
export function fixedOwner(combo: string, platform: string): string | null {
  for (const group of fixedOn(platform)) {
    for (const item of group.items) {
      if (keysOn(item.keys, platform)!.includes(combo)) return item.title
    }
  }
  return null
}

/** Which keys each changeable shortcut is on, by id. Every combination is normalized and appears at most once. */
export type Bindings = ReadonlyMap<string, readonly string[]>

export function defaultBindings(platform: string): Map<string, string[]> {
  return new Map(shortcutsOn(platform).map((shortcut) => [shortcut.id, [...keysOn(shortcut.keys, platform)!]]))
}

/**
 * What `state.json` keeps: the keys of every shortcut the user has changed,
 * by id, and nothing for the ones left alone — so that a key draftpad moves in
 * a later version reaches everybody who never touched it.
 */
export type Overrides = Readonly<Record<string, readonly string[]>>

/**
 * The keys every shortcut is on, from what the user changed.
 *
 * A key the user gave a shortcut is that shortcut's, even when a later
 * version of draftpad has put another shortcut on it by default. Anything the
 * file holds that could not have come from the panel — an id that no longer
 * exists, a key that is not one, a fixed key, a key given twice — is dropped
 * rather than trusted.
 */
export function resolveBindings(overrides: Overrides, platform: string): Map<string, string[]> {
  const bindings = defaultBindings(platform)
  const taken = new Set<string>()
  const clean = (keys: readonly string[]): string[] => {
    const kept: string[] = []
    for (const key of keys) {
      const combo = normalizeCombo(key)
      if (!combo || !isAssignable(combo) || fixedOwner(combo, platform) || taken.has(combo)) continue
      taken.add(combo)
      kept.push(combo)
    }
    return kept
  }
  for (const [id, keys] of bindings) {
    const own = overrides[id]
    if (Array.isArray(own)) bindings.set(id, clean(own))
    else bindings.set(id, keys)
  }
  for (const [id, keys] of bindings) {
    if (!Array.isArray(overrides[id])) bindings.set(id, keys.filter((key) => !taken.has(key)))
  }
  return bindings
}

/** The overrides that give `bindings`: an entry for every shortcut whose keys differ from its defaults. */
export function overridesFor(bindings: Bindings, platform: string): Record<string, string[]> {
  const defaults = defaultBindings(platform)
  const overrides: Record<string, string[]> = {}
  for (const [id, keys] of bindings) {
    const initial = defaults.get(id)
    if (!initial) continue
    if (keys.length !== initial.length || keys.some((key, index) => key !== initial[index])) overrides[id] = [...keys]
  }
  return overrides
}

/** The shortcut a combination is on, by id, or null. */
export function ownerOf(bindings: Bindings, combo: string): string | null {
  for (const [id, keys] of bindings) if (keys.includes(combo)) return id
  return null
}
