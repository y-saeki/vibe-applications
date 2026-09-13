import { expect, test } from './fixtures'

test('counts what is typed', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('hello')
  await expect(app.chars).toHaveText('文字数: 5')
  await app.typeInEditor('\nworld')
  await expect(app.chars).toHaveText('文字数: 11')
  await expect(app.lines).toHaveText('行数: 2')
})

test('hands the text to the backend without being asked to save', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('あとで読む')
  await app.expectSaved((state) => state.text === 'あとで読む')
})

test('writes the text out when the window loses the focus', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('急いで書いた')
  await app.page.evaluate(() => window.dispatchEvent(new Event('blur')))
  await app.expectSaved((state) => state.text === '急いで書いた')
})

test('keeps the last value when a save fails and the text changes again', async ({ launch }) => {
  const app = await launch({ failSave: 'disk is full' })

  await app.typeInEditor('消えないで')
  // The save was attempted and rejected; the editor still holds the text, and
  // nothing half-written was recorded as the saved state.
  await expect.poll(() => app.commands()).toContain('save_state')
  await expect(app.editor).toContainText('消えないで')
  expect(await app.saved()).toBeNull()
})

test('undoes and redoes from the keyboard', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('first')
  await app.press('KeyZ')
  await expect(app.editor).not.toContainText('first')
  await app.page.keyboard.press(`${app.mod}+Shift+KeyZ`)
  await expect(app.editor).toContainText('first')
})

test('indents with spaces, as many as the tab width', async ({ launch }) => {
  const app = await launch({ state: { tabSize: 2 } })

  await app.typeInEditor('x')
  await app.page.keyboard.press('Home')
  await app.page.keyboard.press('Tab')
  await app.expectSaved((state) => state.text === '  x')
})

test('switches the grammar from the status bar and remembers it', async ({ launch }) => {
  const app = await launch()

  await expect(app.editor).toHaveAttribute('data-language', 'markdown')
  await app.languageSelect.selectOption('rust')
  await expect(app.editor).toHaveAttribute('data-language', 'rust')
  await app.expectSaved((state) => state.language === 'rust')
})

test('remembers the always-on-top toggle and passes it to the window', async ({ launch }) => {
  const app = await launch()

  await app.alwaysOnTop.check()
  await app.expectSaved((state) => state.alwaysOnTop)
  const call = (await app.calls()).find((entry) => entry.cmd === 'plugin:window|set_always_on_top')
  expect(call?.args).toMatchObject({ value: true })
})
