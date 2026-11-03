// The tag picker that opens from a card's タグ button.

import { card, expect, ME, test } from './fixtures'

const LABELS = [
  { id: 'Label_1', name: '仕事' },
  { id: 'Label_2', name: '請求書' },
  { id: 'Label_3', name: 'Travel' },
]

test('lists the labels and tags the thread with the one picked', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1), card(2)]], labels: LABELS })
  await app.tool('t1', 'tag').click()
  await expect(app.tagPicker).toBeVisible()
  await expect(app.tagFilter).toBeFocused()
  await expect(app.tagPicker.locator('.tag-row')).toHaveText(['仕事', '請求書', 'Travel'])
  await app.tagPicker.getByText('請求書').click()

  await expect(app.tagPicker).toBeHidden()
  await expect(app.card('t1')).toHaveCount(0)
  expect(await app.callsTo('apply_label')).toEqual([{ account: ME, messageIds: ['m1'], labelId: 'Label_2', mailbox: 'inbox' }])
})

test('typing narrows the list, and Enter takes the highlighted row', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1)]], labels: LABELS })
  await app.tool('t1', 'tag').click()
  await app.tagFilter.fill('tra')
  await expect(app.tagPicker.locator('.tag-row')).toHaveText(['Travel', '「tra」を作成'])
  await app.page.keyboard.press('Enter')
  expect(await app.callsTo('apply_label')).toEqual([{ account: ME, messageIds: ['m1'], labelId: 'Label_3', mailbox: 'inbox' }])
})

test('a new name creates the label first', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1), card(2)]], labels: LABELS })
  await app.tool('t1', 'tag').click()
  await app.tagFilter.fill('新しいタグ')
  await expect(app.tagPicker.locator('.tag-row')).toHaveText(['「新しいタグ」を作成'])
  await app.page.keyboard.press('Enter')
  await expect(app.card('t1')).toHaveCount(0)
  expect(await app.callsTo('create_label')).toEqual([{ account: ME, name: '新しいタグ' }])
  expect(await app.callsTo('apply_label')).toEqual([{ account: ME, messageIds: ['m1'], labelId: 'Label_新しいタグ', mailbox: 'inbox' }])
})

test('an exact name offers no create row', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1)]], labels: LABELS })
  await app.tool('t1', 'tag').click()
  await app.tagFilter.fill('travel')
  await expect(app.tagPicker.locator('.tag-row')).toHaveText(['Travel'])
})

test('Escape and a click outside close it without tagging', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1)]], labels: LABELS })
  await app.tool('t1', 'tag').click()
  await app.page.keyboard.press('Escape')
  await expect(app.tagPicker).toBeHidden()
  await app.tool('t1', 'tag').click()
  await expect(app.tagPicker).toBeVisible()
  await app.count.click()
  await expect(app.tagPicker).toBeHidden()
  expect(await app.callsTo('apply_label')).toEqual([])
  await expect(app.card('t1')).toBeVisible()
})

test('offers the labels of the account the card is in', async ({ launch }) => {
  const app = await launch({
    accounts: [ME, 'work@example.com'],
    inboxes: [[card(1), card(2, { account: 'work@example.com' })]],
    labels: LABELS,
  })
  await app.tool('t2', 'tag').click()
  await app.tagPicker.getByText('仕事').click()
  await expect(app.card('t2')).toHaveCount(0)
  expect(await app.callsTo('list_labels')).toEqual(
    expect.arrayContaining([{ account: ME }, { account: 'work@example.com' }]),
  )
  expect(await app.callsTo('apply_label')).toEqual([
    { account: 'work@example.com', messageIds: ['m2'], labelId: 'Label_1', mailbox: 'inbox' },
  ])
})
