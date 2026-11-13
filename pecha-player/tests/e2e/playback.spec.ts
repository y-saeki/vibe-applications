// The run itself, on Playwright's fake clock: the page clock only moves when a
// case moves it, so "20 seconds later" is exact.

import { expect, test, type App } from './fixtures'

async function ready(app: App, pages = 21): Promise<void> {
  await app.loadPdf(pages)
  await app.expectPhase('ready')
  await app.expectPage(1)
  await app.page.clock.pauseAt(Date.now() + 1000)
}

test('Start turns to the first slide and counts it as 1', async ({ launch }) => {
  const app = await launch()
  await ready(app)
  await app.primary.click()
  await app.expectPhase('running')
  await app.expectPage(2)
  await expect(app.counter).toHaveText('1 / 20')
  await expect(app.clock).toHaveText('0:00')
  await expect(app.primary).toHaveText('ストップ')
  await expect(app.progress).toBeVisible()
})

test('a slide turns every 20 seconds, and the clock counts up', async ({ launch }) => {
  const app = await launch()
  await ready(app)
  await app.primary.click()
  await app.expectPage(2)

  await app.page.clock.runFor(19_000)
  await app.expectPage(2)
  await expect(app.clock).toHaveText('0:19')

  await app.page.clock.runFor(1_000)
  await app.expectPage(3)
  await expect(app.counter).toHaveText('2 / 20')
  await expect(app.clock).toHaveText('0:20')

  await app.page.clock.runFor(45_000)
  await app.expectPage(5)
  await expect(app.counter).toHaveText('4 / 20')
  await expect(app.clock).toHaveText('1:05')
})

test('after the 20th slide has had its 20 seconds, the shutter closes', async ({ launch }) => {
  const app = await launch()
  await ready(app)
  await app.primary.click()

  await app.page.clock.runFor(19 * 20_000 + 19_000)
  await app.expectPage(21)
  await expect(app.counter).toHaveText('20 / 20')
  await app.expectPhase('running')

  await app.page.clock.runFor(1_000)
  await app.expectPhase('closed')
  await expect(app.root).toHaveAttribute('data-end', 'finished')
  await expect(app.endTitle).toHaveText('おつかれさまでした')
  await expect(app.endDetail).toHaveText('20 / 20 枚 · 6:40')
  await expect(app.primary).toHaveText('もう一度')
})

test('a short deck ends after its last slide', async ({ launch }) => {
  const app = await launch()
  await ready(app, 6)
  await app.primary.click()
  await app.page.clock.runFor(5 * 20_000)
  await app.expectPhase('closed')
  await expect(app.endDetail).toHaveText('5 / 5 枚 · 1:40')
})

test('Stop closes the shutter where the run stands', async ({ launch }) => {
  const app = await launch()
  await ready(app)
  await app.primary.click()
  await app.page.clock.runFor(47_000)
  await app.primary.click()
  await app.expectPhase('closed')
  await expect(app.root).toHaveAttribute('data-end', 'stopped')
  await expect(app.endTitle).toHaveText('ストップしました')
  await expect(app.endDetail).toHaveText('3 / 20 枚 · 0:47')

  // Nothing moves behind the shutter.
  await app.page.clock.runFor(60_000)
  await expect(app.endDetail).toHaveText('3 / 20 枚 · 0:47')
  await expect(app.counter).toHaveText('3 / 20')
})

test('もう一度 opens the shutter on the cover', async ({ launch }) => {
  const app = await launch()
  await ready(app)
  await app.primary.click()
  await app.page.clock.runFor(30_000)
  await app.primary.click()
  await app.expectPhase('closed')
  await app.primary.click()
  await app.expectPhase('ready')
  await app.expectPage(1)
  await expect(app.counter).toHaveText('0 / 20')
  await expect(app.clock).toHaveText('0:00')
})

test('Space starts a run but never stops one; Escape does', async ({ launch }) => {
  const app = await launch()
  await ready(app)
  await app.page.keyboard.press('Space')
  await app.expectPhase('running')
  await app.page.clock.runFor(1_000)
  await app.page.keyboard.press('Space')
  await app.page.keyboard.press('Enter')
  await app.expectPhase('running')
  await app.page.keyboard.press('Escape')
  await app.expectPhase('closed')
  await app.page.keyboard.press('Space')
  await app.expectPhase('ready')
})

test('loading is out of reach during a run', async ({ launch }) => {
  const app = await launch()
  await ready(app)
  await app.primary.click()
  await expect(app.loadButton).toBeDisabled()
  await app.page.keyboard.press('ControlOrMeta+o')
  await expect(app.loader).toBeHidden()
})

test('the clock, counter and progress bar can each be hidden, and stay so', async ({ launch }) => {
  const app = await launch()
  await ready(app)
  await app.page.locator('#toggle-clock').click()
  await app.page.locator('#toggle-counter').click()
  await app.page.locator('#toggle-progress').click()
  await expect(app.page.locator('#toggle-clock')).toHaveAttribute('aria-pressed', 'false')
  await app.primary.click()
  await expect(app.clock).toBeHidden()
  await expect(app.counter).toBeHidden()
  await expect(app.progress).toBeHidden()

  await app.page.reload()
  await app.expectPhase('empty')
  await expect(app.page.locator('#toggle-clock')).toHaveAttribute('aria-pressed', 'false')
  await expect(app.page.locator('#toggle-counter')).toHaveAttribute('aria-pressed', 'false')
  await expect(app.page.locator('#toggle-progress')).toHaveAttribute('aria-pressed', 'false')
})

test('the clock and counter sit in the control bar, clear of the slide', async ({ launch }) => {
  const app = await launch()
  await ready(app)
  const bar = app.page.locator('#bar')
  await expect(bar.locator('#clock')).toBeVisible()
  await expect(bar.locator('#counter')).toBeVisible()
  await expect(bar.locator('#progress')).toHaveCount(1)
  await expect(app.page.locator('#stage #clock, #stage #counter, #stage #progress')).toHaveCount(0)
})

// On macOS the window goes to simple fullscreen, which has no title bar to
// slide down over the slide; elsewhere it is the ordinary fullscreen.
const fullscreenCommand = (userAgent: string) =>
  userAgent.includes('Mac') ? 'plugin:window|set_simple_fullscreen' : 'plugin:window|set_fullscreen'

test('the fullscreen button asks the window to go fullscreen, and back', async ({ launch }) => {
  const app = await launch()
  const cmd = fullscreenCommand(await app.page.evaluate(() => navigator.userAgent))
  await app.page.locator('#fullscreen').click()
  await expect(app.root).toHaveAttribute('data-fullscreen', '')
  await app.page.locator('#fullscreen').click()
  await expect(app.root).not.toHaveAttribute('data-fullscreen', '')
  expect((await app.calls()).filter((c) => c.cmd.includes('fullscreen') && !c.cmd.includes('is_'))).toEqual([
    { cmd, args: { label: 'main', value: true } },
    { cmd, args: { label: 'main', value: false } },
  ])
})

test('the control bar stays in fullscreen; only the pointer hides', async ({ launch }) => {
  const app = await launch()
  await ready(app)
  await app.page.locator('#fullscreen').click()
  await expect(app.root).toHaveAttribute('data-fullscreen', '')
  await app.primary.click()
  await app.page.clock.runFor(3_000)
  await expect(app.root).toHaveAttribute('data-idle', '')
  await expect(app.page.locator('#bar')).toBeVisible()
  await expect(app.clock).toBeVisible()
})
