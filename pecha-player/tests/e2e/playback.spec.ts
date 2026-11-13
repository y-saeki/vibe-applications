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

test('the fullscreen button asks for the slide to fill the screen, and back', async ({ launch }) => {
  const app = await launch()
  await app.page.locator('#fullscreen').click()
  await expect(app.root).toHaveAttribute('data-fullscreen', '')
  await app.page.locator('#fullscreen').click()
  await expect(app.root).not.toHaveAttribute('data-fullscreen', '')
  expect((await app.calls()).filter((c) => c.cmd.includes('fullscreen') && !c.cmd.includes('is_'))).toEqual([
    { cmd: 'set_slide_fullscreen', args: { on: true } },
    { cmd: 'set_slide_fullscreen', args: { on: false } },
  ])
})

async function fullscreenRun(app: App): Promise<void> {
  await ready(app)
  await app.page.locator('#fullscreen').click()
  await expect(app.root).toHaveAttribute('data-fullscreen', '')
  await app.page.keyboard.press('Space')
  await app.expectPhase('running')
  // Away from the bar, so that it is free to hide.
  await app.page.mouse.move(400, 100)
}

test('in fullscreen the bar and the pointer hide when idle', async ({ launch }) => {
  const app = await launch()
  await fullscreenRun(app)
  await expect(app.root).not.toHaveAttribute('data-bar-hidden', '')
  await app.page.clock.runFor(3_000)
  await expect(app.root).toHaveAttribute('data-bar-hidden', '')
  await expect(app.root).toHaveAttribute('data-idle', '')
  // The slide is not resized as the bar goes.
  const stage = await app.page.locator('#stage').boundingBox()
  expect(stage?.height).toBe(app.page.viewportSize()?.height)
})

test('in fullscreen, moving the pointer brings back only the pointer, unless it nears the bar', async ({ launch }) => {
  const app = await launch()
  await fullscreenRun(app)
  await app.page.clock.runFor(3_000)
  await expect(app.root).toHaveAttribute('data-bar-hidden', '')

  await app.page.mouse.move(400, 200)
  await expect(app.root).not.toHaveAttribute('data-idle', '')
  await expect(app.root).toHaveAttribute('data-bar-hidden', '')

  const height = app.page.viewportSize()!.height
  await app.page.mouse.move(400, height - 70)
  await expect(app.root).not.toHaveAttribute('data-bar-hidden', '')
})

test('in fullscreen the bar stays while the pointer is on it', async ({ launch }) => {
  const app = await launch()
  await fullscreenRun(app)
  await app.page.locator('#toggle-clock').hover()
  await app.page.clock.runFor(10_000)
  await expect(app.root).not.toHaveAttribute('data-bar-hidden', '')
  await expect(app.root).not.toHaveAttribute('data-idle', '')

  await app.page.mouse.move(400, 100)
  await app.page.clock.runFor(3_000)
  await expect(app.root).toHaveAttribute('data-bar-hidden', '')
})

test('in fullscreen a mousemove where the pointer did not move does not keep the bar up', async ({ launch }) => {
  const app = await launch()
  await fullscreenRun(app)
  // WebKit fires these whenever the page under the pointer changes, such as
  // the clock ticking.
  for (let i = 0; i < 3; i++) {
    await app.page.clock.runFor(1_000)
    await app.page.evaluate(() =>
      window.dispatchEvent(new MouseEvent('mousemove', { clientX: 400, clientY: 100, bubbles: true })),
    )
  }
  await expect(app.root).toHaveAttribute('data-bar-hidden', '')
})
