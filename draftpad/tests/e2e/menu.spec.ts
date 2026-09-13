// On macOS the same commands arrive as a "menu" event from the native menu bar
// (src-tauri/src/menu.rs) instead of as a key press.

import { expect, test } from './fixtures'

const MAC = { platform: 'macos' } as const

test('the menu opens the preferences panel', async ({ launch }) => {
  const app = await launch(MAC)

  await app.runMenuCommand('preferences')
  await expect(app.preferences).toBeVisible()
})

test('the menu opens search and replace', async ({ launch }) => {
  const app = await launch(MAC)

  await app.runMenuCommand('find')
  await expect(app.searchPanel).toBeVisible()
})

test('the menu steps the font size', async ({ launch }) => {
  const app = await launch({ ...MAC, state: { fontSize: 13 } })

  await app.runMenuCommand('increase_font_size')
  await expect(app.page.locator('.cm-editor')).toHaveCSS('font-size', '14px')
  await app.expectSaved((state) => state.fontSize === 14)
})

test('the menu undoes and redoes the draft, not the webview', async ({ launch }) => {
  const app = await launch(MAC)

  await app.typeInEditor('取り消す文字')
  await app.runMenuCommand('undo')
  await expect(app.editor).not.toContainText('取り消す文字')
  await app.runMenuCommand('redo')
  await expect(app.editor).toContainText('取り消す文字')
})

test('the menu leaves the draft alone while preferences has the keyboard', async ({ launch }) => {
  const app = await launch(MAC)

  await app.typeInEditor('残るはず')
  await app.runMenuCommand('preferences')
  await app.runMenuCommand('undo')
  await expect(app.editor).toContainText('残るはず')
})

test('the menu writes the draft out before quitting', async ({ launch }) => {
  const app = await launch(MAC)

  await app.typeInEditor('保存されるはず')
  await app.runMenuCommand('quit')
  await expect.poll(() => app.commands()).toContain('quit_app')
  expect((await app.saved())?.text).toBe('保存されるはず')
})

test('no in-app shortcut handler competes with the menu bar', async ({ launch }) => {
  const app = await launch(MAC)

  // Cmd+, is a menu accelerator on macOS; the window must not act on it too.
  await app.page.keyboard.press('Meta+Comma')
  await expect(app.preferences).toBeHidden()
})
