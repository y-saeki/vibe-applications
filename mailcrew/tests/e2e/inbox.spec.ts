// How the inbox lays its threads out.

import type { Locator, Page } from '@playwright/test'
import { attachment, card, cards, expect, LOGO, ME, test } from './fixtures'

test('lays the threads out newest first, left to right', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(5)] })
  await expect(app.cards).toHaveCount(5)
  expect(await app.ids()).toEqual(['t1', 't2', 't3', 't4', 't5'])
  const lefts = await app.cards.evaluateAll((elements) => elements.map((e) => e.getBoundingClientRect().left))
  expect([...lefts].sort((a, b) => a - b)).toEqual(lefts)
  await expect(app.count).toHaveText('5 件')
})

test('shows the sender, subject and what the thread holds', async ({ launch }) => {
  const app = await launch({
    inboxes: [
      [
        card(1, {
          from: { name: '', email: 'bare@example.com' },
          subject: '',
          messageCount: 3,
          attachments: [attachment('1', 'a.pdf'), attachment('2', 'b.png')],
          unread: true,
        }),
      ],
    ],
  })
  const first = app.card('t1')
  await expect(first.locator('.card-from')).toHaveText('bare@example.com')
  await expect(first.locator('.card-subject')).toHaveText('(件名なし)')
  await expect(first.locator('.card-message-count')).toHaveText('3 通')
  await expect(first.locator('.tool-attachments')).toHaveText('2')
  await expect(first).toHaveClass(/card-unread/)
})

test('every toolbar sits on the same line', async ({ launch }) => {
  const app = await launch({
    inboxes: [
      [
        card(1, { subject: '短い件名' }),
        card(2, { subject: 'とても長い件名が二行にわたって折り返されるくらいの長さで続いていくスレッドの件名', messageCount: 4 }),
        card(3, { body: { kind: 'text', text: 'テキスト\n'.repeat(200) }, attachments: [attachment('1', 'x.zip', { openable: false })] }),
      ],
    ],
  })
  await expect(app.cards).toHaveCount(3)
  const tops = await app.page
    .locator('.card[data-thread-id] .card-toolbar')
    .evaluateAll((toolbars) => toolbars.map((toolbar) => Math.round(toolbar.getBoundingClientRect().top)))
  expect(new Set(tops).size).toBe(1)
})

test("every card's head is the same height, whatever it holds", async ({ launch }) => {
  const app = await launch({
    inboxes: [
      [
        card(1, { subject: '短い件名' }),
        card(2, {
          subject: 'とても長い件名が二行にわたって折り返されるくらいの長さで続いていく、さらにもう少し長いスレッドの件名です',
          messageCount: 4,
          attachments: [attachment('1', 'a.pdf')],
          brand: { bimi: 'pass', domain: 'shop.example', mark: 'vmc', logo: LOGO },
        }),
        card(3, { subject: '', brand: { bimi: 'none', domain: '', mark: 'none', logo: null } }),
      ],
    ],
  })
  await expect(app.cards).toHaveCount(3)
  // Once the bodies, the checks and the logo are in.
  await expect(app.card('t2').locator('.card-logo')).toBeVisible()
  await expect.poll(() => app.trust('t3').then((items) => items.length)).toBe(5)
  const heights = await app.page
    .locator('.card[data-thread-id] .card-head')
    .evaluateAll((heads) => heads.map((head) => Math.round(head.getBoundingClientRect().height)))
  expect(new Set(heights).size).toBe(1)
})

test('a card is half again as wide as it used to be, and narrows only with the window', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1)]] })
  await app.page.setViewportSize({ width: 1280, height: 800 })
  expect(await app.card('t1').evaluate((element) => element.getBoundingClientRect().width)).toBe(570)
  await app.page.setViewportSize({ width: 480, height: 800 })
  const width = await app.card('t1').evaluate((element) => element.getBoundingClientRect().width)
  expect(width).toBeLessThanOrEqual(480)
})

test('ends the strip with the button that clears it', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(3)] })
  await expect(app.cards).toHaveCount(3)
  await expect(app.doneAll).toBeAttached()
  const last = await app.strip.evaluate((strip) => strip.lastElementChild?.classList.contains('card-end'))
  expect(last).toBe(true)
})

test('an empty inbox says so', async ({ launch }) => {
  const app = await launch({ inboxes: [[]] })
  await expect(app.empty).toBeVisible()
  await expect(app.empty).toContainText('受信トレイは空です')
  await expect(app.doneAll).toHaveCount(0)
})

test('reload fetches the inbox again', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1)], [card(9), card(1)]] })
  await expect(app.cards).toHaveCount(1)
  await app.page.locator('#reload').click()
  await expect(app.cards).toHaveCount(2)
  expect(await app.ids()).toEqual(['t9', 't1'])
})

/** How much of the pull-to-reload zone before the first card is in sight. */
function pulledOut(strip: Locator): Promise<number> {
  return strip.evaluate((element) => {
    const zone = element.querySelector('.pull-zone')!
    const rest = (zone.nextElementSibling as HTMLElement).offsetLeft - (zone as HTMLElement).offsetLeft
    return Math.max(0, Math.round(rest - element.scrollLeft))
  })
}

/** Starts noting the zone's labels, in the order they are shown, for `labels`. */
function noteLabels(page: Page): Promise<void> {
  return page.evaluate(() => {
    const label = document.querySelector<HTMLElement>('.pull-label')!
    const seen = [label.textContent!]
    ;(window as unknown as { labels: string[] }).labels = seen
    new MutationObserver(() => {
      if (seen.at(-1) !== label.textContent) seen.push(label.textContent!)
    }).observe(label, { childList: true, characterData: true, subtree: true })
  })
}

function labels(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { labels: string[] }).labels)
}

/**
 * A trackpad gesture as WebView2 sees it: wheel events every 16 ms over
 * `target` (the strip, or a message's frame document when `inFrame`) while the
 * strip scrolls to each of `stops` in turn (or stays put, for a `holdMs`), then
 * nothing. Events arrive with no pause, so whatever the strip does along the
 * way is one gesture.
 */
function wheelGesture(strip: Locator, stops: (number | { holdMs: number })[], inFrame = false): Promise<void> {
  return strip.evaluate(
    async (element, { stops, inFrame }) => {
      const target: EventTarget = inFrame
        ? element.querySelector<HTMLIFrameElement>('iframe')!.contentDocument!
        : element
      const tick = () => new Promise((resolve) => setTimeout(resolve, 16))
      for (const stop of stops) {
        if (typeof stop !== 'number') {
          // Wheel events keep coming while the strip stays where it is.
          for (let held = 0; held < stop.holdMs; held += 16) {
            target.dispatchEvent(new WheelEvent('wheel', { deltaX: -1, bubbles: true }))
            await tick()
          }
          continue
        }
        const from = element.scrollLeft
        for (let step = 1; step <= 6; step++) {
          target.dispatchEvent(new WheelEvent('wheel', { deltaX: (stop - from) / 6, bubbles: true }))
          element.scrollTo({ left: from + ((stop - from) * step) / 6 })
          await tick()
        }
      }
    },
    { stops, inFrame },
  )
}

/** What AppKit reports through `trackpad.rs`: fingers touching or leaving the trackpad. */
function fingers(page: Page, state: 'down' | 'up'): Promise<void> {
  return page.evaluate((payload) => window.__mailcrew.emit!('trackpad', payload), state)
}

/**
 * Scrolls the strip as momentum or fingers would, without a wheel event, and
 * waits for the page to have heard of it.
 */
function scrollStrip(strip: Locator, left: number): Promise<void> {
  return strip.evaluate(async (element, to) => {
    const scrolled = new Promise((resolve) => element.addEventListener('scroll', resolve, { once: true }))
    element.scrollTo({ left: to })
    await scrolled
  }, left)
}

const RELOADING = ['引っ張って再読み込み', '離すと再読み込み', '読み込み中']

test('pulling the strip right from its first card reloads once the wheel stops', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1)], [card(9), card(1)]] })
  await expect(app.cards).toHaveCount(1)
  // Hidden even when a single card leaves the strip room to spare.
  await expect.poll(() => pulledOut(app.strip)).toBe(0)
  await expect(app.page.locator('.pull-zone')).toHaveAttribute('data-armed', 'true')
  await noteLabels(app.page)
  await wheelGesture(app.strip, [0])
  await expect(app.cards).toHaveCount(2)
  expect(await app.ids()).toEqual(['t9', 't1'])
  expect((await labels(app.page)).slice(0, 3)).toEqual(RELOADING)
  // Laid out again from the first card, with the pull zone out of sight.
  await expect.poll(() => pulledOut(app.strip)).toBe(0)
})

test("a wheel pull over a message's body reaches the page in WebView2 only", async ({ launch, browserName }) => {
  const app = await launch({
    inboxes: [[card(1, { body: { kind: 'html', html: '<p>本文</p>', remoteImages: false } })], [card(9), card(1)]],
  })
  await expect(app.card('t1').locator('iframe')).toBeVisible()
  await expect(app.page.locator('.pull-zone')).toHaveAttribute('data-armed', 'true')
  await wheelGesture(app.strip, [0], true)
  if (browserName === 'webkit') {
    // WKWebView runs none of the page's listeners inside the frame, so wheel
    // events there never count; on macOS the trackpad events from the Rust
    // side stand in for them.
    await expect.poll(() => pulledOut(app.strip)).toBe(0)
    await app.expectCalls('list_inbox', 1)
  } else {
    await expect(app.cards).toHaveCount(2)
  }
})

test('momentum that carries a fling back to the start springs back without reloading', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(5)] })
  await expect(app.cards).toHaveCount(5)
  await scrollStrip(app.strip, 900)
  await expect(app.page.locator('.pull-zone')).toHaveAttribute('data-armed', 'false')
  await app.page.waitForTimeout(300)
  // One gesture from far along that comes to rest on the first card long enough
  // to count as at rest, then coasts on to the start.
  const rest = await app.strip.evaluate((element) => {
    const zone = element.querySelector<HTMLElement>('.pull-zone')!
    return (zone.nextElementSibling as HTMLElement).offsetLeft - zone.offsetLeft
  })
  await wheelGesture(app.strip, [rest, { holdMs: 300 }, 0])
  await expect.poll(() => pulledOut(app.strip)).toBe(0)
  await app.page.waitForTimeout(300)
  await app.expectCalls('list_inbox', 1)
})

test('pulling the zone out and back before letting go does not reload', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(3)] })
  await expect(app.cards).toHaveCount(3)
  await expect(app.page.locator('.pull-zone')).toHaveAttribute('data-armed', 'true')
  const rest = await app.strip.evaluate((element) => element.scrollLeft)
  await wheelGesture(app.strip, [0, rest / 2])
  await expect.poll(() => pulledOut(app.strip)).toBe(0)
  await app.page.waitForTimeout(300)
  await app.expectCalls('list_inbox', 1)
})

test('a short swipe at the start of the strip springs back without reloading', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(3)] })
  await expect(app.cards).toHaveCount(3)
  await app.card('t1').locator('.card-head').hover()
  await app.page.mouse.wheel(-30, 0)
  await expect.poll(() => pulledOut(app.strip)).toBe(0)
  await app.expectCalls('list_inbox', 1)
})

test('a mouse wheel stops at the first card rather than reloading', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(3)] })
  await expect(app.cards).toHaveCount(3)
  await app.card('t1').locator('.card-head').hover()
  await app.page.mouse.wheel(0, -400)
  await app.page.waitForTimeout(300)
  await app.expectCalls('list_inbox', 1)
  await expect.poll(() => pulledOut(app.strip)).toBe(0)
})

test('on macOS a pull is held while the fingers stay down, and reloads when they lift', async ({ launch }) => {
  const app = await launch({ platform: 'macos', inboxes: [cards(3), [card(9), ...cards(3)]] })
  await expect(app.cards).toHaveCount(3)
  await expect(app.page.locator('.pull-zone')).toHaveAttribute('data-armed', 'true')
  await noteLabels(app.page)
  await fingers(app.page, 'down')
  await scrollStrip(app.strip, 0)
  await expect(app.page.locator('.pull-label')).toHaveText('離すと再読み込み')
  // Resting at the start without lifting is not letting go.
  await app.page.waitForTimeout(400)
  await app.expectCalls('list_inbox', 1)
  await fingers(app.page, 'up')
  await expect(app.cards).toHaveCount(4)
  expect((await labels(app.page)).slice(0, 3)).toEqual(RELOADING)
})

test('on macOS momentum carrying the strip to the start never reloads', async ({ launch }) => {
  const app = await launch({ platform: 'macos', inboxes: [cards(5)] })
  await expect(app.cards).toHaveCount(5)
  // A fling from far along: the fingers lifted long before the strip got here.
  await fingers(app.page, 'down')
  await scrollStrip(app.strip, 900)
  await fingers(app.page, 'up')
  // It comes to rest on the first card long enough to count as at rest, then
  // coasts on to the start.
  await app.strip.evaluate(async (element) => {
    const zone = element.querySelector<HTMLElement>('.pull-zone')!
    element.scrollTo({ left: (zone.nextElementSibling as HTMLElement).offsetLeft - zone.offsetLeft })
    await new Promise((resolve) => setTimeout(resolve, 300))
    element.scrollTo({ left: 0 })
  })
  await expect.poll(() => pulledOut(app.strip)).toBe(0)
  await app.page.waitForTimeout(300)
  await app.expectCalls('list_inbox', 1)
})

test('on macOS fingers that came down away from the first card do not pull', async ({ launch }) => {
  const app = await launch({ platform: 'macos', inboxes: [cards(5)] })
  await expect(app.cards).toHaveCount(5)
  await scrollStrip(app.strip, 900)
  await expect(app.page.locator('.pull-zone')).toHaveAttribute('data-armed', 'false')
  await app.page.waitForTimeout(300)
  await fingers(app.page, 'down')
  // Dragged back to the first card, held there, then on to the start, all
  // without lifting.
  await app.strip.evaluate(async (element) => {
    const zone = element.querySelector<HTMLElement>('.pull-zone')!
    element.scrollTo({ left: (zone.nextElementSibling as HTMLElement).offsetLeft - zone.offsetLeft })
    await new Promise((resolve) => setTimeout(resolve, 300))
  })
  await scrollStrip(app.strip, 0)
  await fingers(app.page, 'up')
  await expect.poll(() => pulledOut(app.strip)).toBe(0)
  await app.page.waitForTimeout(300)
  await app.expectCalls('list_inbox', 1)
})

test('the cards follow a pull only part of the way', async ({ launch }) => {
  const app = await launch({ platform: 'macos', inboxes: [cards(3)] })
  await expect(app.cards).toHaveCount(3)
  await expect(app.page.locator('.pull-zone')).toHaveAttribute('data-armed', 'true')
  // Fingers held down throughout, so the pull is never let go.
  await fingers(app.page, 'down')
  const { pulled, moved } = await app.strip.evaluate(async (strip) => {
    const frame = () => new Promise((resolve) => requestAnimationFrame(resolve))
    const first = strip.querySelector<HTMLElement>('.card[data-thread-id]')!
    const rest = strip.scrollLeft
    const before = first.getBoundingClientRect().left
    strip.scrollTo({ left: 0 })
    await frame()
    await frame()
    const moved = first.getBoundingClientRect().left - before
    strip.scrollTo({ left: rest })
    return { pulled: rest, moved }
  })
  await fingers(app.page, 'up')
  // 80px (--pull-lag) less than the strip was pulled.
  expect(Math.round(moved)).toBe(Math.round(pulled) - 80)
  await app.expectCalls('list_inbox', 1)
})

for (const [platform, modifier] of [
  ['windows', 'Control'],
  ['macos', 'Meta'],
] as const) {
  test(`${modifier}+R on ${platform} fetches the inbox rather than reloading the page`, async ({ launch }) => {
    const app = await launch({ platform, inboxes: [[card(1)], [card(2)]] })
    await expect(app.cards).toHaveCount(1)
    await app.page.keyboard.press(`${modifier}+KeyR`)
    await expect(app.card('t2')).toBeVisible()
    await app.expectCalls('list_inbox', 2)
    await app.expectCalls('get_session', 1)
  })
}

test('a card does not name the account while there is only one', async ({ launch }) => {
  const one = await launch({ inboxes: [[card(1)]] })
  await expect(one.card('t1').locator('.card-account')).toBeHidden()
})

test('lays out the threads of every account together, as the Rust side orders them', async ({ launch }) => {
  const app = await launch({
    accounts: [ME, 'work@example.com'],
    inboxes: [[card(1, { account: 'work@example.com' }), card(2), card(3, { account: 'work@example.com' })]],
  })
  await expect(app.cards).toHaveCount(3)
  expect(await app.ids()).toEqual(['t1', 't2', 't3'])
  await expect(app.card('t1').locator('.card-account')).toHaveText('work@example.com')
  await expect(app.card('t2').locator('.card-account')).toHaveText(ME)
  expect(await app.callsTo('list_inbox')).toEqual([{ accounts: [], mailbox: 'inbox', query: '' }])
})

test('an account that did not answer is named, and the others are shown', async ({ launch }) => {
  const app = await launch({
    accounts: [ME, 'work@example.com'],
    inboxes: [[card(1)]],
    inboxFailures: [{ account: 'work@example.com', kind: 'network', message: 'Google に接続できませんでした' }],
  })
  await expect(app.cards).toHaveCount(1)
  await expect(app.notice).toHaveText('work@example.com: Google に接続できませんでした')
})

test('a failed load says why', async ({ launch }) => {
  const app = await launch({ failures: { list_inbox: { kind: 'network', message: 'Google に接続できませんでした' } } })
  await expect(app.notice).toHaveText('Google に接続できませんでした')
})
