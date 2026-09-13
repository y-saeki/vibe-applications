// Keyboard shortcuts are handled inside the app on Windows, where there is no
// menu bar. `src/commands.ts` is the single table both paths resolve against.

import { expect, test } from './fixtures'

test('Ctrl+F opens search and replace', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await expect(app.searchPanel).toBeHidden()
  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()
  await expect(app.searchPanel.getByPlaceholder('検索')).toBeVisible()
})

test('Ctrl+, opens the preferences panel', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await app.press('Comma')
  await expect(app.preferences).toBeVisible()
})

test('Ctrl+= and Ctrl+- step the font size, within the limits', async ({ launch }) => {
  const app = await launch({ platform: 'windows', state: { fontSize: 13 } })

  await app.press('Equal')
  await expect(app.page.locator('.cm-editor')).toHaveCSS('font-size', '14px')
  await app.press('Minus')
  await app.press('Minus')
  await expect(app.page.locator('.cm-editor')).toHaveCSS('font-size', '12px')
  await app.expectSaved((state) => state.fontSize === 12)
})

test('the font size stops at the smallest allowed value', async ({ launch }) => {
  const app = await launch({ platform: 'windows', state: { fontSize: 10 } })

  await app.press('Minus')
  await expect(app.page.locator('.cm-editor')).toHaveCSS('font-size', '10px')
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
