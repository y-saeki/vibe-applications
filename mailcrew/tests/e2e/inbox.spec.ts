// How the inbox lays its threads out.

import { card, cards, expect, test } from './fixtures'

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
        }),
        card(3, { subject: '' }),
      ],
    ],
  })
  await expect(app.cards).toHaveCount(3)
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

test('a failed load says why', async ({ launch }) => {
  const app = await launch({ failures: { list_inbox: { kind: 'network', message: 'Google に接続できませんでした' } } })
  await expect(app.notice).toHaveText('Google に接続できませんでした')
})
