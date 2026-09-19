// On macOS the same commands arrive as a "menu" event from the native menu bar
// (src-tauri/src/menu.rs) instead of as a key press.

import { expect, test } from './fixtures'

const MAC = { platform: 'macos' } as const

test('the menu opens the preferences panel', async ({ launch }) => {
  const app = await launch(MAC)

  await app.runMenuCommand('preferences')
  await expect(app.preferences).toBeVisible()
})

test('the menu steps the font size', async ({ launch }) => {
  const app = await launch({ ...MAC, state: { fontSize: 13 } })

  await app.runMenuCommand('increase_font_size')
  await expect(app.pane('a')).toHaveCSS('font-size', '14px')
  await expect(app.pane('b')).toHaveCSS('font-size', '14px')
  await app.expectSaved((state) => state.fontSize === 14)
})

test('the menu undoes and redoes the pane that has the keyboard, not the webview', async ({ launch }) => {
  const app = await launch({ ...MAC, state: { textA: 'left' } })

  await app.typeInEditor('b', '取り消す文字')
  await app.runMenuCommand('undo')
  await expect(app.editorB).not.toContainText('取り消す文字')
  await expect(app.editorA).toContainText('left')
  await app.runMenuCommand('redo')
  await expect(app.editorB).toContainText('取り消す文字')
})

test('the menu leaves the panes alone while preferences has the keyboard', async ({ launch }) => {
  const app = await launch(MAC)

  await app.typeInEditor('a', '残るはず')
  await app.runMenuCommand('preferences')
  await app.runMenuCommand('undo')
  await expect(app.editorA).toContainText('残るはず')
})

test('the menu steps through the chunks', async ({ launch }) => {
  const app = await launch({ ...MAC, state: { textA: 'one\ntwo\nthree\nfour', textB: 'one\nTWO\nthree\nFOUR' } })

  await app.runMenuCommand('next_diff')
  expect(await app.caretLine('a')).toBe(2)
  await app.runMenuCommand('next_diff')
  expect(await app.caretLine('a')).toBe(4)
  await app.runMenuCommand('previous_diff')
  expect(await app.caretLine('a')).toBe(2)
})

test('the menu writes both texts out before quitting', async ({ launch }) => {
  const app = await launch(MAC)

  await app.typeInEditor('a', '左は残る')
  await app.typeInEditor('b', '右も残る')
  await app.runMenuCommand('quit')
  await expect.poll(() => app.commands()).toContain('quit_app')
  const saved = await app.saved()
  expect(saved?.textA).toBe('左は残る')
  expect(saved?.textB).toBe('右も残る')
})

test('no in-app shortcut handler competes with the menu bar', async ({ launch }) => {
  const app = await launch({ ...MAC, state: { textA: 'one\ntwo', textB: 'one\nTWO' } })

  // Cmd+, and F7 are menu accelerators on macOS; the window must not act on
  // them too.
  await app.page.keyboard.press('Meta+Comma')
  await expect(app.preferences).toBeHidden()
  await app.page.keyboard.press('F7')
  expect(await app.caretLine('a')).toBe(1)
})
