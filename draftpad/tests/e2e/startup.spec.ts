import { expect, test } from './fixtures'

test('carries the previous text over and counts it', async ({ launch }) => {
  const app = await launch({ state: { text: 'おはよう\n世界🌏' } })

  await expect(app.editor).toContainText('おはよう')
  // Code points, not UTF-16 units: the emoji counts as one character.
  await expect(app.chars).toHaveText('8 文字')
  await expect(app.lines).toHaveText('2 行')
})

test('restores the saved settings', async ({ launch }) => {
  const app = await launch({
    state: {
      text: 'a b\n    c',
      language: 'rust',
      fontSize: 24,
      alwaysOnTop: true,
      theme: 'dark',
      showWhitespace: true,
      showIndentGuides: true,
      showLineNumbers: true,
    },
  })

  await expect(app.languageSelect).toHaveValue('rust')
  await expect(app.alwaysOnTop).toHaveAttribute('aria-pressed', 'true')
  await expect(app.page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await expect(app.editor).toHaveAttribute('data-language', 'rust')
  // The space on the first line and the four that indent the second, each
  // marked on its own; the second line is the one level the rules are for.
  await expect(app.whitespaceMarks).toHaveCount(5)
  await expect(app.indentGuides).toHaveCount(1)
  await expect(app.lineNumberCells).toHaveText(['1', '2'])
  // The window setting is the backend's to apply.
  expect(await app.commands()).toContain('plugin:window|set_always_on_top')
})

test('starts at the saved font weight, snapped onto the scale the panel offers', async ({ launch }) => {
  const app = await launch({ state: { fontWeight: 460 } })

  await expect(app.editor).toHaveCSS('font-weight', '500')
  await app.gear.click()
  await expect(app.page.locator('#pref-font-weight')).toHaveValue('500')
})

test('restores the search toggles', async ({ launch }) => {
  const app = await launch({
    state: { searchCaseSensitive: true, searchRegexp: true },
  })

  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()
  await expect(app.searchMatchCase).toBeChecked()
  await expect(app.searchRegexp).toBeChecked()
})

test('follows the OS when the theme is "system"', async ({ launch }) => {
  const app = await launch({ state: { theme: 'system' }, colorScheme: 'dark' })

  await expect(app.page.locator('html')).toHaveAttribute('data-theme', 'dark')
})

test('tells the backend which platform it is running on', async ({ launch }) => {
  const app = await launch({ platform: 'macos' })

  await expect(app.page.locator('html')).toHaveAttribute('data-platform', 'macos')
})

test('puts the caret in the editor before showing the window', async ({ launch }) => {
  const app = await launch()

  // Windows anchors the IME candidate list to the caret, so the window must not
  // become visible while nothing inside it is focused.
  await expect(app.editor).toBeFocused()
  const commands = await app.commands()
  expect(commands.filter((cmd) => cmd === 'plugin:window|show')).toHaveLength(1)
})

test('opens the preferences panel when the jump list task started it', async ({ launch }) => {
  const app = await launch({ openPreferences: true })

  await expect(app.preferences).toBeVisible()
})

test('reports a failure to load instead of showing an empty editor', async ({ launch }) => {
  const app = await launch({ failLoad: 'state.json is unreadable' })

  await expect(app.page.locator('body')).toContainText('draftpad の起動に失敗しました')
  await expect(app.page.locator('body')).toContainText('state.json is unreadable')
})
