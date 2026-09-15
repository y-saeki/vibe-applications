import { type App, expect, test } from './fixtures'

test('counts what is typed', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('hello')
  await expect(app.chars).toHaveText('5 文字')
  await app.typeInEditor('\nworld')
  await expect(app.chars).toHaveText('11 文字')
  await expect(app.lines).toHaveText('2 行')
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

test('keeps the list shut until it is asked for', async ({ launch }) => {
  const app = await launch()
  const firstOption = app.languageSelect.locator('option').first()

  // The browser hides a closed picker with `display: none`, and a `display`
  // written for it in style.css beats that: the list then sits open over the
  // window from the moment the app loads, on every select at once. Nothing
  // else here would notice, because every other case opens the list first.
  await expect(firstOption).toBeHidden()

  await app.languageSelect.click()
  // A list the platform owns is a window of its own, which never enters the
  // page whether it is open or not.
  if (await app.listIsOurs()) await expect(firstOption).toBeVisible()
})

test('leaves the list to macOS', async ({ launch }) => {
  const app = await launch({ platform: 'macos' })

  // The menu macOS opens already looks like the rest of that system, so the
  // base appearance is asked for on Windows only. Without saying so, an engine
  // new enough would take the list over there on its own.
  expect(await app.listIsOurs()).toBe(false)
})

test('picks a language from the dropped-open list', async ({ launch }) => {
  const app = await launch()
  const styleable = await app.page.evaluate(() => CSS.supports('appearance', 'base-select'))
  // Without the base appearance the list is the platform's own window, which
  // nothing inside the page can reach. The case above covers the same choice
  // arriving through the DOM, which is the path that works on both.
  test.skip(!styleable, 'this engine opens the platform list, not one the page can drive')

  await app.languageSelect.click()
  const rust = app.languageSelect.locator('option', { hasText: 'Rust' })
  await expect(rust).toBeVisible()
  await rust.click()

  await expect(app.editor).toHaveAttribute('data-language', 'rust')
  await app.expectSaved((state) => state.language === 'rust')
})

test('walks the dropped-open list with the keyboard', async ({ launch }) => {
  const app = await launch()
  test.skip(!(await app.listIsOurs()), 'the platform owns the list here, so the page cannot drive it')

  // The list is laid out by this sheet once the engine takes the base
  // appearance, so its own styling is what could stop the rows taking the
  // focus. The keyboard is the path that shows it.
  await app.languageSelect.click()
  await expect(app.languageSelect.locator('option').first()).toBeVisible()
  // Two rows down from Markdown: plain text is the one in between, and it
  // carries no grammar, so nothing would be left to assert on the editor.
  await app.page.keyboard.press('ArrowDown')
  await app.page.keyboard.press('ArrowDown')
  await app.page.keyboard.press('Enter')

  await expect(app.editor).toHaveAttribute('data-language', 'yaml')
  await app.expectSaved((state) => state.language === 'yaml')
})

test('remembers the search toggles once they are switched', async ({ launch }) => {
  const app = await launch()

  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()
  await app.searchMatchCase.check()

  await app.expectSaved((state) => state.searchCaseSensitive && !state.searchRegexp)
})

test('remembers the always-on-top toggle and passes it to the window', async ({ launch }) => {
  const app = await launch()

  await app.alwaysOnTop.click()
  await expect(app.alwaysOnTop).toHaveAttribute('aria-pressed', 'true')
  await app.expectSaved((state) => state.alwaysOnTop)
  const call = (await app.calls()).find((entry) => entry.cmd === 'plugin:window|set_always_on_top')
  expect(call?.args).toMatchObject({ value: true })
})

test('works the always-on-top toggle from the keyboard, and says which way it is', async ({ launch }) => {
  const app = await launch({ state: { alwaysOnTop: true } })

  // It is a button rather than a checkbox, so the state a screen reader reads
  // out is aria-pressed and nothing else says it.
  await expect(app.alwaysOnTop).toHaveAttribute('aria-pressed', 'true')
  await app.alwaysOnTop.focus()
  await app.page.keyboard.press('Space')
  await expect(app.alwaysOnTop).toHaveAttribute('aria-pressed', 'false')
  await app.expectSaved((state) => !state.alwaysOnTop)

  await app.page.keyboard.press('Enter')
  await expect(app.alwaysOnTop).toHaveAttribute('aria-pressed', 'true')
})

test('keeps the bar still however many digits the counts run to', async ({ launch }) => {
  // 111,999 characters over 1,000 lines: six digits and four, the widest
  // readings the two cells are built for.
  const app = await launch({ state: { text: Array.from({ length: 1000 }, () => 'あ'.repeat(111)).join('\n') } })

  await expect(app.chars).toHaveText('111999 文字')
  await expect(app.lines).toHaveText('1000 行')
  // Each cell is held at the width its token gives it, separator included, so
  // the longest reading does not push it wider than the shortest one.
  expect((await app.chars.boundingBox())?.width).toBe(await token(app, '--count-width'))
  expect((await app.lines.boundingBox())?.width).toBe(await token(app, '--count-width-narrow'))

  // Which is what the bar is really being asked for: emptying the draft takes
  // the counts from their widest reading to their shortest, and nothing to the
  // right of them moves.
  const wide = await app.languageSelect.boundingBox()
  await app.press('KeyA')
  await app.page.keyboard.press('Backspace')
  await expect(app.chars).toHaveText('0 文字')
  expect((await app.languageSelect.boundingBox())?.x).toBe(wide?.x)
})

/** A length token as the sheet declares it, in px. */
function token(app: App, name: string): Promise<number> {
  return app.page.evaluate(
    (property) => Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue(property)),
    name,
  )
}
