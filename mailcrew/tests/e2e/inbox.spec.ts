// How the inbox lays its threads out.

import type { Locator, Page } from '@playwright/test'
import { card, cards, expect, LOGO, ME, test } from './fixtures'

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
          attachments: ['a.pdf', 'b.png'],
          unread: true,
        }),
      ],
    ],
  })
  const first = app.card('t1')
  await expect(first.locator('.card-from')).toHaveText('bare@example.com')
  await expect(first.locator('.card-subject')).toHaveText('(件名なし)')
  await expect(first.locator('.card-message-count')).toHaveText('3 通')
  await expect(first.locator('.card-attachments')).toHaveText('2')
  await expect(first).toHaveClass(/card-unread/)
})

test('every toolbar sits on the same line', async ({ launch }) => {
  const app = await launch({
    inboxes: [
      [
        card(1, { subject: '短い件名' }),
        card(2, { subject: 'とても長い件名が二行にわたって折り返されるくらいの長さで続いていくスレッドの件名', messageCount: 4 }),
        card(3, { body: { kind: 'text', text: 'テキスト\n'.repeat(200) }, attachments: ['x.zip'] }),
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
          attachments: ['a.pdf'],
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
    const rest = zone.nextElementSibling!.getBoundingClientRect().left - zone.getBoundingClientRect().left
    return Math.max(0, Math.round(rest - element.scrollLeft))
  })
}

/**
 * Swipes right at the start of the strip and waits for the second page. When
 * it does not come, the failure lists where the strip scrolled on the way.
 */
async function swipeToReload(page: Page, cards: Locator): Promise<void> {
  await page.evaluate(() => {
    const strip = document.querySelector<HTMLElement>('#strip')!
    const zone = strip.querySelector<HTMLElement>('.pull-zone')!
    const trace: string[] = [`start ${strip.scrollLeft} armed=${zone.dataset.armed}`]
    ;(window as unknown as { pullTrace: string[] }).pullTrace = trace
    strip.addEventListener('scroll', () => trace.push(`${strip.scrollLeft} armed=${zone.dataset.armed}`))
  })
  await page.mouse.wheel(-400, 0)
  try {
    await expect(cards).toHaveCount(2)
  } catch (err) {
    const trace = await page.evaluate(() => (window as unknown as { pullTrace: string[] }).pullTrace)
    throw new Error(`${(err as Error).message}\nscroll trace: ${trace.join(' | ')}`)
  }
}

test('swiping right at the start of the strip reloads', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1)], [card(9), card(1)]] })
  await expect(app.cards).toHaveCount(1)
  // Hidden even when a single card leaves the strip room to spare.
  await expect.poll(() => pulledOut(app.strip)).toBe(0)
  await expect(app.page.locator('.pull-zone')).toHaveAttribute('data-armed', 'true')
  // Scrolled straight to the start: one synthetic wheel event in WebKit moves
  // the strip only a step and snaps back, where a trackpad keeps pulling.
  const after = await app.strip.evaluate((strip) => {
    const zone = strip.querySelector<HTMLElement>('.pull-zone')!
    const trace: string[] = []
    ;(window as unknown as { pullTrace: string[] }).pullTrace = trace
    strip.addEventListener('scroll', () => trace.push(`${strip.scrollLeft} armed=${zone.dataset.armed}`))
    const before = `${strip.scrollLeft} max=${strip.scrollWidth - strip.clientWidth} zone=${zone.offsetLeft}`
    strip.scrollTo({ left: 0 })
    return `${before} -> ${strip.scrollLeft}`
  })
  try {
    await expect(app.cards).toHaveCount(2)
  } catch (err) {
    const trace = await app.page.evaluate(() => (window as unknown as { pullTrace: string[] }).pullTrace)
    throw new Error(`${(err as Error).message}\nscrollTo: ${after}\nscroll trace: ${trace.join(' | ')}`)
  }
  expect(await app.ids()).toEqual(['t9', 't1'])
  // Laid out again from the first card, with the pull zone out of sight.
  await expect.poll(() => pulledOut(app.strip)).toBe(0)
})

test("the swipe reloads over a message's body too", async ({ launch }) => {
  const app = await launch({
    inboxes: [[card(1, { body: { kind: 'html', html: '<p>本文</p>', remoteImages: false } })], [card(9), card(1)]],
  })
  const frame = app.card('t1').locator('iframe')
  await expect(frame).toBeVisible()
  await frame.hover()
  await expect(app.page.locator('.pull-zone')).toHaveAttribute('data-armed', 'true')
  await swipeToReload(app.page, app.cards)
})

test('a short swipe at the start of the strip springs back without reloading', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(3)] })
  await expect(app.cards).toHaveCount(3)
  await app.card('t1').locator('.card-head').hover()
  await app.page.mouse.wheel(-30, 0)
  await expect.poll(() => pulledOut(app.strip)).toBe(0)
  await app.expectCalls('list_inbox', 1)
})

test('a swipe that runs past the first card does not reload', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(5)] })
  await expect(app.cards).toHaveCount(5)
  await app.card('t1').locator('.card-head').hover()
  await app.page.mouse.wheel(300, 0)
  await expect.poll(() => app.strip.evaluate((strip) => strip.scrollLeft)).toBeGreaterThan(200)
  // Straight back without stopping at the first card, as a fling does.
  await app.page.mouse.wheel(-2000, 0)
  await expect.poll(() => pulledOut(app.strip)).toBe(0)
  await expect(app.card('t1')).toBeInViewport()
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
