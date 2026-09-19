// Keyboard shortcuts are handled inside the app on Windows, where there is no
// menu bar. `src/commands.ts` is the single table both paths resolve against.

import { expect, test } from './fixtures'

test('Ctrl+F opens search and replace, with the keyboard on the find field', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await expect(app.searchPanel).toBeHidden()
  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()
  await expect(app.searchField).toBeFocused()

  // What the panel is for: the next thing typed is the search term, not text
  // appended to the draft.
  await app.page.keyboard.type('alpha')
  await expect(app.searchField).toHaveValue('alpha')
  await expect(app.editor).not.toContainText('alpha')
})

test('Ctrl+F puts the keyboard back on the find field once the panel is open', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await app.press('KeyF')
  await expect(app.searchField).toBeFocused()
  await app.editor.click()
  await expect(app.editor).toBeFocused()

  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()
  await expect(app.searchField).toBeFocused()
})

// Two of the controls CodeMirror puts in the panel are taken out in
// src/style.css: "select all matches", whose command needs the multiple
// selections draftpad does not enable, and the whole-word toggle, which never
// matches Japanese prose. The toggle is hidden by its position among the
// labels, so the two that stay are checked here as well.
test('the search panel leaves out the select-all button and the whole-word toggle', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await app.press('KeyF')
  await expect(app.searchPanel.getByRole('button', { name: '次へ' })).toBeVisible()
  await expect(app.searchPanel.locator('button[name="select"]')).toBeHidden()
  await expect(app.searchPanel.locator('label:has(input[name="case"])')).toBeVisible()
  await expect(app.searchPanel.locator('label:has(input[name="re"])')).toBeVisible()
  await expect(app.searchPanel.locator('label:has(input[name="word"])')).toBeHidden()
})

// draftpad holds nothing but plain text, so this is the same paste Ctrl+V
// already does. It is bound because the habit comes from editors where the two
// differ, and pressing it here used to do nothing at all.
test('Ctrl+Shift+V pastes the clipboard at the cursor', async ({ launch }) => {
  const app = await launch({ platform: 'windows', clipboard: '貼り付けた文字' })

  await app.typeInEditor('前後')
  await app.page.keyboard.press('ArrowLeft')
  await app.page.keyboard.press(`${app.mod}+Shift+KeyV`)
  await app.expectSaved((state) => state.text === '前貼り付けた文字後')
})

test('Ctrl+Shift+V replaces the selection, and undoes in one step', async ({ launch }) => {
  const app = await launch({ platform: 'windows', clipboard: '新しい下書き' })

  await app.typeInEditor('古い下書き')
  await app.press('KeyA')
  await app.page.keyboard.press(`${app.mod}+Shift+KeyV`)
  await app.expectSaved((state) => state.text === '新しい下書き')

  await app.press('KeyZ')
  await app.expectSaved((state) => state.text === '古い下書き')
})

test('Ctrl+Shift+V leaves the draft alone when the clipboard holds no text', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await app.typeInEditor('そのまま')
  await app.page.keyboard.press(`${app.mod}+Shift+KeyV`)
  await expect.poll(() => app.commands()).toContain('plugin:clipboard-manager|read_text')
  await expect(app.editor).toHaveText('そのまま')
})

// The search panel and the preferences panel both take the keyboard off the
// editor, and the clipboard belongs wherever the caret is. Dropping it into the
// draft behind them is not what the key press asked for.
test('Ctrl+Shift+V leaves the draft alone while the search panel has the keyboard', async ({ launch }) => {
  const app = await launch({ platform: 'windows', clipboard: '割り込み' })

  await app.typeInEditor('そのまま')
  await app.press('KeyF')
  await expect(app.searchField).toBeFocused()
  await app.page.keyboard.press(`${app.mod}+Shift+KeyV`)
  expect(await app.commands()).not.toContain('plugin:clipboard-manager|read_text')
  await expect(app.editor).toHaveText('そのまま')
})

test('Ctrl+F leaves the search panel closed while preferences has the keyboard', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await app.press('Comma')
  await expect(app.preferences).toBeVisible()

  // The panel would open behind the preferences panel, where the keyboard
  // cannot reach it.
  await app.press('KeyF')
  await expect(app.searchPanel).toBeHidden()
})

test('Ctrl+, opens the preferences panel', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await app.press('Comma')
  await expect(app.preferences).toBeVisible()
})

test('F11 asks the window for fullscreen', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await app.page.keyboard.press('F11')
  await expect.poll(() => app.commands()).toContain('plugin:window|set_fullscreen')
  const call = (await app.calls()).filter((entry) => entry.cmd === 'plugin:window|set_fullscreen').pop()
  expect(call?.args).toMatchObject({ value: true })
})

test('Ctrl+Q writes the draft out before quitting', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await app.typeInEditor('未送信の下書き')
  await app.press('KeyQ')
  await expect.poll(() => app.commands()).toContain('quit_app')

  const commands = await app.commands()
  expect(commands.lastIndexOf('save_state')).toBeLessThan(commands.indexOf('quit_app'))
  expect((await app.saved())?.text).toBe('未送信の下書き')
})

test('closing the window goes through the same path as quitting', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await app.typeInEditor('閉じる前に残す')
  await app.press('KeyW')
  await expect.poll(() => app.commands()).toContain('quit_app')
  expect((await app.saved())?.text).toBe('閉じる前に残す')
})

test('a window resize is remembered, in logical pixels', async ({ launch }) => {
  const app = await launch({ platform: 'windows', innerSize: { width: 1000, height: 750 }, scaleFactor: 2 })

  await app.page.evaluate(() => window.dispatchEvent(new Event('resize')))
  await app.expectSaved((state) => state.windowWidth === 500 && state.windowHeight === 375)
})
