// Keyboard shortcuts are handled inside the app on Windows, where there is no
// menu bar. `src/commands.ts` is the single table both paths resolve against.

import { expect, test } from './fixtures'

test('Ctrl+, opens the preferences panel', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await app.press('Comma')
  await expect(app.preferences).toBeVisible()
})

test('Ctrl+= and Ctrl+- step the font size of both panes, within the limits', async ({ launch }) => {
  const app = await launch({ platform: 'windows', state: { fontSize: 13 } })

  await app.press('Equal')
  await expect(app.pane('a')).toHaveCSS('font-size', '14px')
  await expect(app.pane('b')).toHaveCSS('font-size', '14px')
  await app.press('Minus')
  await app.press('Minus')
  await expect(app.pane('a')).toHaveCSS('font-size', '12px')
  await expect(app.pane('b')).toHaveCSS('font-size', '12px')
  await app.expectSaved((state) => state.fontSize === 12)
})

test('the font size stops at the smallest allowed value', async ({ launch }) => {
  const app = await launch({ platform: 'windows', state: { fontSize: 10 } })

  await app.press('Minus')
  await expect(app.pane('a')).toHaveCSS('font-size', '10px')
})

test('F7 and Shift+F7 step through the chunks', async ({ launch }) => {
  const app = await launch({ platform: 'windows', state: { textA: 'one\ntwo\nthree\nfour', textB: 'one\nTWO\nthree\nFOUR' } })

  await app.page.keyboard.press('F7')
  expect(await app.caretLine('a')).toBe(2)
  await app.page.keyboard.press('F7')
  expect(await app.caretLine('a')).toBe(4)
  await app.page.keyboard.press('Shift+F7')
  expect(await app.caretLine('a')).toBe(2)
})

test('F11 asks the window for fullscreen', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await app.page.keyboard.press('F11')
  await expect.poll(() => app.commands()).toContain('plugin:window|set_fullscreen')
  const call = (await app.calls()).filter((entry) => entry.cmd === 'plugin:window|set_fullscreen').pop()
  expect(call?.args).toMatchObject({ value: true })
})

test('Ctrl+Q writes both texts out before quitting', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await app.typeInEditor('a', '左の下書き')
  await app.typeInEditor('b', '右の下書き')
  await app.press('KeyQ')
  await expect.poll(() => app.commands()).toContain('quit_app')

  const commands = await app.commands()
  expect(commands.lastIndexOf('save_state')).toBeLessThan(commands.indexOf('quit_app'))
  const saved = await app.saved()
  expect(saved?.textA).toBe('左の下書き')
  expect(saved?.textB).toBe('右の下書き')
})

test('closing the window goes through the same path as quitting', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  await app.typeInEditor('a', '閉じる前に残す')
  await app.press('KeyW')
  await expect.poll(() => app.commands()).toContain('quit_app')
  expect((await app.saved())?.textA).toBe('閉じる前に残す')
})

test('a window resize is remembered, in logical pixels', async ({ launch }) => {
  const app = await launch({ platform: 'windows', innerSize: { width: 1000, height: 750 }, scaleFactor: 2 })

  await app.page.evaluate(() => window.dispatchEvent(new Event('resize')))
  await app.expectSaved((state) => state.windowWidth === 500 && state.windowHeight === 375)
})
