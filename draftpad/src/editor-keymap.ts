// The editor's keymap: CodeMirror's own bindings, with the ones the
// preferences panel can move taken out and put back on whatever keys the user
// gave them.
//
// Which bindings those are is `src/shortcuts.ts`'s list; this file knows the
// CodeMirror command behind each id. A stock binding is taken out by the
// command it runs rather than by its key, so that it goes whichever of its
// platform variants (`mac:`, `win:`) it was written with.

import { closeBracketsKeymap } from '@codemirror/autocomplete'
import {
  addCursorAbove,
  addCursorBelow,
  copyLineDown,
  copyLineUp,
  cursorMatchingBracket,
  cursorSyntaxLeft,
  cursorSyntaxRight,
  defaultKeymap,
  deleteLine,
  historyKeymap,
  indentLess,
  indentMore,
  indentSelection,
  indentWithTab,
  insertBlankLine,
  moveLineDown,
  moveLineUp,
  redo,
  redoSelection,
  selectLine,
  selectParentSyntax,
  selectSyntaxLeft,
  selectSyntaxRight,
  toggleBlockComment,
  toggleComment,
  toggleTabFocusMode,
  undoSelection,
} from '@codemirror/commands'
import { findNext, findPrevious, gotoLine, openSearchPanel, searchKeymap, selectNextOccurrence, selectSelectionMatches } from '@codemirror/search'
import type { Command, KeyBinding } from '@codemirror/view'

import { toCodeMirror } from './keys'
import type { Bindings } from './shortcuts'

interface EditorCommand {
  run: Command
  /** What the same key runs with Shift held, as CodeMirror's own binding had it. */
  shift?: Command
  /** Where the key works; the editor alone when left out. */
  scope?: string
}

/** Where the search panel's own keys work: in the draft, and in the panel's fields. */
const SEARCH_SCOPE = 'editor search-panel'

/** The editor commands in `SHORTCUTS`, by id. */
const EDITOR_COMMANDS: Record<string, EditorCommand> = {
  move_line_up: { run: moveLineUp },
  move_line_down: { run: moveLineDown },
  copy_line_up: { run: copyLineUp },
  copy_line_down: { run: copyLineDown },
  delete_line: { run: deleteLine },
  insert_blank_line: { run: insertBlankLine },
  indent_more: { run: indentMore },
  indent_less: { run: indentLess },
  indent_selection: { run: indentSelection },
  toggle_comment: { run: toggleComment },
  toggle_block_comment: { run: toggleBlockComment },
  select_line: { run: selectLine },
  select_next: { run: selectNextOccurrence },
  select_parent: { run: selectParentSyntax },
  undo_selection: { run: undoSelection },
  redo_selection: { run: redoSelection },
  syntax_left: { run: cursorSyntaxLeft, shift: selectSyntaxLeft },
  syntax_right: { run: cursorSyntaxRight, shift: selectSyntaxRight },
  matching_bracket: { run: cursorMatchingBracket },
  find_next: { run: findNext, scope: SEARCH_SCOPE },
  find_previous: { run: findPrevious, scope: SEARCH_SCOPE },
  goto_line: { run: gotoLine },
  tab_focus_mode: { run: toggleTabFocusMode },
}

/**
 * Stock bindings that are not the editor's to answer at all.
 *
 * - Opening the search panel is the application's "find" command
 *   (`src/commands.ts`), which the menu bar or the window's key handler runs
 *   before the editor sees the key; a copy here would go on answering the old
 *   key after it has been moved.
 * - Multiple selections stay off (EditorState.allowMultipleSelections), so a
 *   selection holding several ranges collapses to its main one. Selecting
 *   every match of the selection (searchKeymap's Mod-Shift-l) and adding a
 *   cursor above or below therefore do nothing a plain key does not, and are
 *   not offered. The panel's "all" button goes the same way in style.css.
 */
const DROPPED = new Set<Command>([openSearchPanel, selectSelectionMatches, addCursorAbove, addCursorBelow])

const MANAGED = new Set<Command>(Object.values(EDITOR_COMMANDS).map((command) => command.run))

/** A stock keymap with the managed and the dropped bindings taken out. */
function unmanaged(bindings: readonly KeyBinding[]): KeyBinding[] {
  return bindings.filter((binding) => !(binding.run && (DROPPED.has(binding.run) || MANAGED.has(binding.run))))
}

// historyKeymap binds redo to Mod-y everywhere and to Ctrl-Shift-z on Linux
// only, so Windows needs this one; on macOS it repeats the Cmd-Shift-z binding
// historyKeymap already has.
const redoKeymap: KeyBinding[] = [{ key: 'Mod-Shift-z', run: redo, preventDefault: true }]

const BEFORE_RESTORE = [...closeBracketsKeymap, ...unmanaged(searchKeymap), ...redoKeymap]
// Tab stays on indentMore even though Mod-] is one of the managed bindings:
// indentWithTab is its own binding, and one of the fixed keys.
const AFTER_RESTORE = [...unmanaged(historyKeymap), ...unmanaged(defaultKeymap), indentWithTab]

/**
 * The whole keymap of a pane.
 *
 * The user's keys go first. None of them can be a key a stock binding still
 * answers — the fixed keys cannot be given away — so the order only settles a
 * key CodeMirror answers without draftpad listing it, which is then the
 * user's.
 *
 * @param restoreKey the pane's undo key while a closed pane can come back,
 *   ahead of the history's own
 */
export function editorKeymap(bindings: Bindings, restoreKey: KeyBinding): KeyBinding[] {
  const own: KeyBinding[] = []
  for (const [id, command] of Object.entries(EDITOR_COMMANDS)) {
    for (const combo of bindings.get(id) ?? []) {
      own.push({ key: toCodeMirror(combo), run: command.run, shift: command.shift, scope: command.scope, preventDefault: true })
    }
  }
  return [...own, ...BEFORE_RESTORE, restoreKey, ...AFTER_RESTORE]
}

/** The ids the editor binds itself, as opposed to the application's commands. */
export const EDITOR_COMMAND_IDS: ReadonlySet<string> = new Set(Object.keys(EDITOR_COMMANDS))
