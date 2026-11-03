// The four buttons on a card, and the one at the end of the strip.

import { card, cards, expect, test } from './fixtures'

test('完了 archives the whole thread and takes the card off', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1, { messageIds: ['m0', 'm1'] }), card(2)]] })
  await app.tool('t1', 'done').click()
  await expect(app.card('t1')).toHaveCount(0)
  expect(await app.ids()).toEqual(['t2'])
  await expect(app.count).toHaveText('1 件')
  expect(await app.callsTo('archive')).toEqual([{ messageIds: ['m0', 'm1'] }])
})

test('あとで読む sets the thread aside and takes the card off', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1), card(2)]] })
  await app.tool('t2', 'later').click()
  await expect(app.card('t2')).toHaveCount(0)
  expect(await app.callsTo('set_aside')).toEqual([{ messageIds: ['m2'] }])
})

test('スター stars the shown message and unstars the thread', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1, { messageIds: ['m0', 'm1'] })]] })
  const star = app.tool('t1', 'star')
  await expect(star).toHaveAttribute('aria-pressed', 'false')
  await star.click()
  await expect(star).toHaveAttribute('aria-pressed', 'true')
  await app.expectCalls('set_starred', 1)
  await expect(star).toBeEnabled()
  await star.click()
  await expect(star).toHaveAttribute('aria-pressed', 'false')
  expect(await app.callsTo('set_starred')).toEqual([
    { messageId: 'm1', messageIds: ['m0', 'm1'], starred: true },
    { messageId: 'm1', messageIds: ['m0', 'm1'], starred: false },
  ])
  // Starring keeps the thread in the inbox.
  await expect(app.card('t1')).toBeVisible()
})

test('a failed star goes back to how it was', async ({ launch }) => {
  const app = await launch({
    inboxes: [[card(1)]],
    failures: { set_starred: { kind: 'network', message: 'Google に接続できませんでした' } },
  })
  const star = app.tool('t1', 'star')
  await star.click()
  await expect(app.notice).toHaveText('Google に接続できませんでした')
  await expect(star).toHaveAttribute('aria-pressed', 'false')
})

test('a failed action puts the card back where it was', async ({ launch }) => {
  const app = await launch({
    inboxes: [cards(3)],
    failures: { archive: { kind: 'google', message: 'Gmail がエラーを返しました: x' } },
  })
  await app.tool('t2', 'done').click()
  await expect(app.notice).toHaveText('Gmail がエラーを返しました: x')
  expect(await app.ids()).toEqual(['t1', 't2', 't3'])
  await expect(app.count).toHaveText('3 件')
})

test('clearing the last card fetches the next page', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1)], [card(51), card(52)]] })
  await app.tool('t1', 'done').click()
  await expect(app.cards).toHaveCount(2)
  expect(await app.ids()).toEqual(['t51', 't52'])
})

test('まとめて完了 archives everything on the strip and fetches the next page', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(3), [card(60)]] })
  await expect(app.cards).toHaveCount(3)
  await app.tool('t2', 'done').click()
  await expect(app.cards).toHaveCount(2)
  await app.doneAll.scrollIntoViewIfNeeded()
  await app.doneAll.click()
  await expect(app.cards).toHaveCount(1)
  expect(await app.ids()).toEqual(['t60'])
  expect(await app.callsTo('archive')).toEqual([{ messageIds: ['m2'] }, { messageIds: ['m1', 'm3'] }])
})

test('まとめて完了 shows the empty inbox once nothing is left', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(2), []] })
  await app.doneAll.scrollIntoViewIfNeeded()
  await app.doneAll.click()
  await expect(app.empty).toBeVisible()
  await expect(app.cards).toHaveCount(0)
})

test('a failed まとめて完了 keeps every card', async ({ launch }) => {
  const app = await launch({
    inboxes: [cards(2)],
    failures: { archive: { kind: 'network', message: 'Google に接続できませんでした' } },
  })
  await app.doneAll.scrollIntoViewIfNeeded()
  await app.doneAll.click()
  await expect(app.notice).toHaveText('Google に接続できませんでした')
  await expect(app.cards).toHaveCount(2)
  await expect(app.doneAll).toBeEnabled()
})
