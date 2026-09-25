// The single command table. Menu items (macOS) and keyboard shortcuts
// (Windows) resolve to entries in this list. Ids match the menu item ids in
// src-tauri/src/menu.rs, and the ids in src/shortcuts.ts, which says which key
// each one is on.

import type { Bindings } from './shortcuts'

export interface Command {
  id: string
  title: string
  run: () => void | Promise<void>
}

export interface CommandActions {
  openPreferences: () => void
  undo: () => void
  redo: () => void
  quit: () => Promise<void>
  /** Closes the pane with the caret while there are two, and the window while there is one. */
  close: () => Promise<void>
  openCompare: () => Promise<void>
  toggleFullscreen: () => Promise<void>
  openSearch: () => void
  pastePlain: () => Promise<void>
}

export function createCommands(actions: CommandActions): Command[] {
  const commands: Command[] = [
    { id: 'preferences', title: '環境設定…', run: actions.openPreferences },
    // Undo and redo have no key here: CodeMirror's own keymap already binds them
    // inside the editor, and a window-wide shortcut would take Ctrl+Z away from
    // the text fields in the preferences panel. The menu bar forwards them.
    { id: 'undo', title: '元に戻す', run: actions.undo },
    { id: 'redo', title: 'やり直す', run: actions.redo },
    // draftpad only ever holds plain text, so Cmd/Ctrl+V already pastes without
    // formatting. This is the shortcut other editors give that, bound so the
    // habit pastes here too instead of doing nothing.
    { id: 'paste_plain', title: 'プレーンテキストとして貼り付け', run: actions.pastePlain },
    { id: 'find', title: '検索・置換', run: actions.openSearch },
    // The key VS Code splits its editor with. It only ever opens the pane;
    // closing one is what Mod+W means once there are two.
    { id: 'open_compare', title: 'テキストを比較する', run: actions.openCompare },
    { id: 'toggle_fullscreen', title: 'フルスクリーンを切り替え', run: actions.toggleFullscreen },
    // Closes the innermost thing that can be closed: the pane with the caret
    // while there are two, otherwise the window, which is the app.
    { id: 'close', title: '閉じる', run: actions.close },
    { id: 'quit', title: '終了', run: actions.quit },
  ]
  return commands
}

/**
 * The command each key is on, from the keys src/shortcuts.ts resolved. Keys on
 * the editor's own commands are not in it: those are CodeMirror's to answer.
 */
export function indexByKeys(commands: readonly Command[], bindings: Bindings): Map<string, Command> {
  const index = new Map<string, Command>()
  for (const command of commands) {
    for (const combo of bindings.get(command.id) ?? []) index.set(combo, command)
  }
  return index
}
