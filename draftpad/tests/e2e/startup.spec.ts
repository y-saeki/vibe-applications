import { expect, test } from './fixtures'

test('carries the previous text over and counts it', async ({ launch }) => {
  const app = await launch({ state: { text: 'おはよう\n世界🌏' } })

  await expect(app.editor).toContainText('おはよう')
  // Code points, not UTF-16 units: the emoji counts as one character.
  await expect(app.chars).toHaveText('文字数: 8')
  await expect(app.lines).toHaveText('行数: 2')
})

test('restores the saved settings', async ({ launch }) => {
  const app = await launch({
    state: { language: 'rust', fontSize: 24, alwaysOnTop: true, theme: 'dark' },
  })

  await expect(app.languageSelect).toHaveValue('rust')
  await expect(app.alwaysOnTop).toBeChecked()
  await expect(app.page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await expect(app.editor).toHaveAttribute('data-language', 'rust')
  // The window setting is the backend's to apply.
  expect(await app.commands()).toContain('plugin:window|set_always_on_top')
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
