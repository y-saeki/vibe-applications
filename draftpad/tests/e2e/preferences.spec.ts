import { expect, test } from './fixtures'

test('opens from the gear button and closes with Escape', async ({ launch }) => {
  const app = await launch()

  await expect(app.preferences).toBeHidden()
  await app.gear.click()
  await expect(app.preferences).toBeVisible()

  await app.page.keyboard.press('Escape')
  await expect(app.preferences).toBeHidden()
  // Closing hands the keyboard back, so typing goes into the draft again.
  await expect(app.editor).toBeFocused()
})

test('closes when the backdrop is clicked', async ({ launch }) => {
  const app = await launch()

  await app.gear.click()
  await expect(app.preferences).toBeVisible()
  await app.preferences.click({ position: { x: 4, y: 4 } })
  await expect(app.preferences).toBeHidden()
})

test('shows the version', async ({ launch }) => {
  const app = await launch({ version: '9.8.7' })

  await app.gear.click()
  await expect(app.page.locator('#pref-version')).toHaveText('draftpad 9.8.7')
})

test('switches to the dark look and remembers it', async ({ launch }) => {
  const app = await launch({ colorScheme: 'light' })

  await app.gear.click()
  await app.page.locator('#pref-theme').selectOption('dark')
  await expect(app.page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await app.expectSaved((state) => state.theme === 'dark')
})

test('applies the font size and pulls out-of-range values back in', async ({ launch }) => {
  const app = await launch()
  const fontSize = app.page.locator('#pref-font-size')

  await app.gear.click()
  await fontSize.fill('40')
  await fontSize.blur()
  await expect(app.page.locator('.cm-editor')).toHaveCSS('font-size', '40px')
  await app.expectSaved((state) => state.fontSize === 40)

  await fontSize.fill('999')
  await fontSize.blur()
  await expect(fontSize).toHaveValue('100')
  await app.expectSaved((state) => state.fontSize === 100)
})

test('pulls an out-of-range tab width back in', async ({ launch }) => {
  const app = await launch()
  const tabSize = app.page.locator('#pref-tab-size')

  await app.gear.click()
  await tabSize.fill('0')
  await tabSize.blur()
  await expect(tabSize).toHaveValue('1')
  await app.expectSaved((state) => state.tabSize === 1)
})

test('offers the fonts the backend reports', async ({ launch }) => {
  const app = await launch({ fonts: ['BIZ UDGothic', 'Consolas'] })

  await app.gear.click()
  await expect(app.page.locator('#font-list option')).toHaveCount(2)
  await expect(app.page.locator('#font-list option').first()).toHaveAttribute('value', 'BIZ UDGothic')
})

test('turns Vim mode on, and the editor stops taking plain typing', async ({ launch }) => {
  const app = await launch()

  await app.gear.click()
  await app.page.locator('#pref-mode').selectOption('vim')
  await app.expectSaved((state) => state.editorMode === 'vim')
  await app.page.keyboard.press('Escape')

  await app.typeInEditor('iinserted')
  // Normal mode swallows the leading "i"; the rest is inserted.
  await app.expectSaved((state) => state.text === 'inserted')
})

test('starts in Vim mode when that is what was saved', async ({ launch }) => {
  const app = await launch({ state: { editorMode: 'vim' } })

  await expect(app.page.locator('.cm-scroller')).toHaveClass(/cm-vimMode/)
})
