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
    const rest = zone.nextElementSibling!.getBoundingClientRect().left - zone.getBoundingClientRect().left
    return Math.max(0, Math.round(rest - element.scrollLeft))
  })
}

/**
 * Swipes right at the start of the strip, lets go, and waits for the second
 * page. Resolves with the zone's labels in the order they were shown. When the
 * page does not come, the failure lists where the strip scrolled on the way.
 */
async function swipeToReload(page: Page, cards: Locator): Promise<string[]> {
  await page.evaluate(() => {
    const strip = document.querySelector<HTMLElement>('#strip')!
    const zone = strip.querySelector<HTMLElement>('.pull-zone')!
    const label = zone.querySelector<HTMLElement>('.pull-label')!
    const record = { trace: [`start ${strip.scrollLeft} armed=${zone.dataset.armed}`], labels: [label.textContent!] }
    ;(window as unknown as { pull: typeof record }).pull = record
    strip.addEventListener('scroll', () => record.trace.push(`${strip.scrollLeft} armed=${zone.dataset.armed}`))
    new MutationObserver(() => {
      if (record.labels.at(-1) !== label.textContent) record.labels.push(label.textContent!)
    }).observe(label, { childList: true, characterData: true, subtree: true })
  })
  // A run of small steps, as a trackpad sends them.
  for (let i = 0; i < 8; i++) await page.mouse.wheel(-50, 0)
  const record = () => page.evaluate(() => (window as unknown as { pull: { trace: string[]; labels: string[] } }).pull)
  try {
    await expect(cards).toHaveCount(2)
  } catch (err) {
    throw new Error(`${(err as Error).message}\nscroll trace: ${(await record()).trace.join(' | ')}`)
  }
  return (await record()).labels
}

test('swiping right at the start of the strip reloads', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1)], [card(9), card(1)]] })
  await expect(app.cards).toHaveCount(1)
  // Hidden even when a single card leaves the strip room to spare.
  await expect.poll(() => pulledOut(app.strip)).toBe(0)
  await expect(app.page.locator('.pull-zone')).toHaveAttribute('data-armed', 'true')
  await app.card('t1').locator('.card-head').hover()
  // Says what a pull does, then that letting go reloads, then that it is reloading.
  const labels = await swipeToReload(app.page, app.cards)
  expect(labels.slice(0, 3)).toEqual(['引っ張って再読み込み', '離すと再読み込み', '読み込み中'])
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

test('the cards follow a pull only part of the way', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(3)] })
  await expect(app.cards).toHaveCount(3)
  await expect(app.page.locator('.pull-zone')).toHaveAttribute('data-armed', 'true')
  const { pulled, moved } = await app.strip.evaluate(async (strip) => {
    const frame = () => new Promise((resolve) => requestAnimationFrame(resolve))
    const first = strip.querySelector<HTMLElement>('.card[data-thread-id]')!
    const rest = strip.scrollLeft
    const before = first.getBoundingClientRect().left
    strip.scrollTo({ left: 0 })
    await frame()
    await frame()
    const moved = first.getBoundingClientRect().left - before
    // Put it back before it counts as letting go.
    strip.scrollTo({ left: rest })
    return { pulled: rest, moved }
  })
  // 80px (--pull-lag) less than the strip was pulled.
  expect(Math.round(moved)).toBe(Math.round(pulled) - 80)
  await app.expectCalls('list_inbox', 1)
})

test('pulling the zone out and back before letting go does not reload', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(3)] })
  await expect(app.cards).toHaveCount(3)
  await expect(app.page.locator('.pull-zone')).toHaveAttribute('data-armed', 'true')
  await app.strip.evaluate(async (strip) => {
    const frame = () => new Promise((resolve) => requestAnimationFrame(resolve))
    const rest = strip.scrollLeft
    strip.scrollTo({ left: 0 })
    await frame()
    await frame()
    strip.scrollTo({ left: rest / 2 })
  })
  await expect.poll(() => pulledOut(app.strip)).toBe(0)
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

test('a swipe that runs past the first card does not reload', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(5)] })
  await expect(app.cards).toHaveCount(5)
  await app.card('t1').locator('.card-head').hover()
  for (let i = 0; i < 6; i++) await app.page.mouse.wheel(200, 0)
  await expect.poll(() => app.strip.evaluate((strip) => strip.scrollLeft)).toBeGreaterThan(600)
  // Straight back without stopping at the first card, as a fling does.
  await app.page.mouse.wheel(-4000, 0)
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
