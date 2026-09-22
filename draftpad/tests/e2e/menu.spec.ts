// On macOS the same commands arrive as a "menu" event from the native menu bar
// (src-tauri/src/menu.rs) instead of as a key press.

import { expect, test } from './fixtures'

const MAC = { platform: 'macos' } as const

test('the menu opens the preferences panel', async ({ launch }) => {
  const app = await launch(MAC)

  await app.runMenuCommand('preferences')
  await expect(app.preferences).toBeVisible()
})

test('the menu opens search and replace, with the keyboard on the find field', async ({ launch }) => {
  const app = await launch(MAC)

  await app.runMenuCommand('find')
  await expect(app.searchPanel).toBeVisible()
  await expect(app.searchField).toBeFocused()

  // The menu path has to land on the field just as the key press does, or the
  // search term is typed into the draft.
  await app.page.keyboard.type('alpha')
  await expect(app.searchField).toHaveValue('alpha')
  await expect(app.editor).not.toContainText('alpha')
})

test('the menu pastes the clipboard as plain text', async ({ launch }) => {
  const app = await launch({ ...MAC, clipboard: '貼り付けた文字' })

  await app.typeInEditor('前後')
  await app.page.keyboard.press('ArrowLeft')
  await app.runMenuCommand('paste_plain')
  await app.expectSaved((state) => state.text === '前貼り付けた文字後')
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

test('the menu leaves the search panel closed while preferences has the keyboard', async ({ launch }) => {
  const app = await launch(MAC)

  await app.runMenuCommand('preferences')
  await expect(app.preferences).toBeVisible()

  await app.runMenuCommand('find')
  await expect(app.searchPanel).toBeHidden()
})

test('the menu writes the draft out before quitting', async ({ launch }) => {
  const app = await launch(MAC)

  await app.typeInEditor('保存されるはず')
  await app.runMenuCommand('quit')
  await expect.poll(() => app.commands()).toContain('quit_app')
  expect((await app.saved())?.text).toBe('保存されるはず')
})

test('the menu opens the compare pane', async ({ launch }) => {
  const app = await launch(MAC)

  await app.typeInEditor('比べる')
  await app.runMenuCommand('open_compare')
  await expect(app.page.locator('html')).toHaveAttribute('data-layout', 'compare')
  await expect(app.editorB).toHaveText('比べる')
})

test('the menu closes the pane with the caret while there are two, and the window once there is one', async ({ launch }) => {
  const app = await launch({ ...MAC, state: { compare: true, text: 'left', compareText: 'right' } })

  await app.editorB.click()
  await expect(app.editorB).toBeFocused()
  await app.runMenuCommand('close')
  await expect(app.page.locator('html')).toHaveAttribute('data-layout', 'single')
  await expect(app.editor).toHaveText('left')
  expect(await app.commands()).not.toContain('quit_app')

  await app.runMenuCommand('close')
  await expect.poll(() => app.commands()).toContain('quit_app')
})

test('no in-app shortcut handler competes with the menu bar', async ({ launch }) => {
  const app = await launch(MAC)

  // Cmd+, and Cmd+\ are menu accelerators on macOS; the window must not act
  // on them too.
  await app.page.keyboard.press('Meta+Comma')
  await expect(app.preferences).toBeHidden()
  await app.page.keyboard.press('Meta+Backslash')
  await expect(app.page.locator('html')).toHaveAttribute('data-layout', 'single')
})
