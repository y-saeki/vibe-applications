import { expect, test } from './fixtures'

test('carries both texts over and counts the chunks between them', async ({ launch }) => {
  const app = await launch({ state: { textA: 'おはよう\n世界🌏', textB: 'こんばんは\n世界🌏' } })

  await expect(app.editorA).toContainText('おはよう')
  await expect(app.editorB).toContainText('こんばんは')
  await expect(app.diffCount).toHaveText('差異 1 箇所')
})

test('restores the saved settings', async ({ launch }) => {
  const app = await launch({
    state: { textA: 'a b', diffMode: 'line', fontSize: 24, alwaysOnTop: true, theme: 'dark', showWhitespace: true },
  })

  await expect(app.diffModeSelect).toHaveValue('line')
  await expect(app.page.locator('#editor')).toHaveAttribute('data-diff-mode', 'line')
  await expect(app.pane('a')).toHaveCSS('font-size', '24px')
  await expect(app.pane('b')).toHaveCSS('font-size', '24px')
  await expect(app.alwaysOnTop).toHaveAttribute('aria-pressed', 'true')
  await expect(app.page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await expect(app.whitespaceMarks('a')).toHaveCount(1)
  // The window setting is the backend's to apply.
  expect(await app.commands()).toContain('plugin:window|set_always_on_top')
})

test('starts at the saved font weight, snapped onto the scale the panel offers', async ({ launch }) => {
  const app = await launch({ state: { fontWeight: 460 } })

  await expect(app.editorA).toHaveCSS('font-weight', '500')
  await expect(app.editorB).toHaveCSS('font-weight', '500')
  await app.gear.click()
  await expect(app.page.locator('#pref-font-weight')).toHaveValue('500')
})

test('follows the OS when the theme is "system"', async ({ launch }) => {
  const app = await launch({ state: { theme: 'system' }, colorScheme: 'dark' })

  await expect(app.page.locator('html')).toHaveAttribute('data-theme', 'dark')
})

test('tells the backend which platform it is running on', async ({ launch }) => {
  const app = await launch({ platform: 'macos' })

  await expect(app.page.locator('html')).toHaveAttribute('data-platform', 'macos')
})

test('puts the caret in the left pane before showing the window', async ({ launch }) => {
  const app = await launch()

  // Windows anchors the IME candidate list to the caret, so the window must not
  // become visible while nothing inside it is focused.
  await expect(app.editorA).toBeFocused()
  const commands = await app.commands()
  expect(commands.filter((cmd) => cmd === 'plugin:window|show')).toHaveLength(1)
})

test('says what each pane is for while it is empty', async ({ launch }) => {
  const app = await launch()

  await expect(app.pane('a').locator('.cm-placeholder')).toHaveText('比較元のテキスト')
  await expect(app.pane('b').locator('.cm-placeholder')).toHaveText('比較先のテキスト')
})

test('reports a failure to load instead of showing empty panes', async ({ launch }) => {
  const app = await launch({ failLoad: 'state.json is unreadable' })

  await expect(app.page.locator('body')).toContainText('diffpad の起動に失敗しました')
  await expect(app.page.locator('body')).toContainText('state.json is unreadable')
})
