// The tabs above the strip, and the custom views made in the settings.

import { card, expect, ME, test } from './fixtures'

const WORK = 'work@example.com'

test('starts on the whole inbox across every account', async ({ launch }) => {
  const app = await launch({ accounts: [ME, WORK], inboxes: [[card(1)]] })
  await expect(app.cards).toHaveCount(1)
  await expect(app.tabs).toHaveText(['すべて'])
  await expect(app.tab('すべて')).toHaveAttribute('aria-selected', 'true')
  expect(await app.callsTo('list_inbox')).toEqual([{ accounts: [], query: '' }])
})

test('a view fetches its accounts with its search, and すべて goes back', async ({ launch }) => {
  const app = await launch({
    accounts: [ME, WORK],
    views: [{ id: 'v1', name: '仕事', accounts: [WORK], query: 'is:unread' }],
    inboxes: [[card(1), card(2, { account: WORK })], [card(2, { account: WORK })], [card(1)]],
  })
  await expect(app.tabs).toHaveText(['すべて', '仕事'])
  await expect(app.cards).toHaveCount(2)
  await app.tab('仕事').click()
  await expect(app.cards).toHaveCount(1)
  await expect(app.tab('仕事')).toHaveAttribute('aria-selected', 'true')
  await app.tab('すべて').click()
  await app.expectCalls('list_inbox', 3)
  expect(await app.callsTo('list_inbox')).toEqual([
    { accounts: [], query: '' },
    { accounts: [WORK], query: 'is:unread' },
    { accounts: [], query: '' },
  ])
})

test('an empty view says nothing matches', async ({ launch }) => {
  const app = await launch({ views: [{ id: 'v1', name: '未読', accounts: [], query: 'is:unread' }], inboxes: [[card(1)], []] })
  await expect(app.cards).toHaveCount(1)
  await app.tab('未読').click()
  await expect(app.empty).toBeVisible()
  await expect(app.empty).toContainText('このビューに当てはまるメールはありません')
})

test('a view whose accounts have all been removed shows nothing and asks for nothing', async ({ launch }) => {
  const app = await launch({ views: [{ id: 'v1', name: '旧', accounts: ['gone@example.com'], query: '' }], inboxes: [[card(1)]] })
  await expect(app.cards).toHaveCount(1)
  await app.tab('旧').click()
  await expect(app.empty).toBeVisible()
  await app.expectCalls('list_inbox', 1)
})

test('makes a view in the settings and shows it as a tab', async ({ launch }) => {
  const app = await launch({ accounts: [ME, WORK] })
  await app.openSettings()
  await expect(app.page.locator('#view-list')).toContainText('ビューはまだありません')
  await app.page.locator('#add-view').click()
  await app.page.locator('#view-name').fill('仕事')
  await app.page.locator('#view-accounts').getByLabel(WORK).check()
  await app.page.locator('#view-query').fill('from:boss@example.com')
  await app.page.locator('#save-view').click()

  await expect(app.page.locator('#view-form')).toBeHidden()
  await expect(app.page.locator('#view-list .settings-row-name')).toHaveText(['仕事'])
  await expect(app.page.locator('#view-list .settings-row-summary')).toHaveText(`${WORK} ・ from:boss@example.com`)
  const saved = (await app.callsTo('save_views')) as { views: unknown[] }[]
  expect(saved).toHaveLength(1)
  expect(saved[0]!.views).toEqual([
    { id: expect.any(String), name: '仕事', accounts: [WORK], query: 'from:boss@example.com' },
  ])
  await app.page.locator('#close-settings').click()
  await expect(app.tabs).toHaveText(['すべて', '仕事'])
})

test('a view needs a name', async ({ launch }) => {
  const app = await launch()
  await app.openSettings()
  await app.page.locator('#add-view').click()
  await app.page.locator('#save-view').click()
  await expect(app.page.locator('#view-error')).toHaveText('ビューの名前を入力してください')
  expect(await app.callsTo('save_views')).toEqual([])
})

test('edits and deletes a view', async ({ launch }) => {
  const app = await launch({
    accounts: [ME, WORK],
    views: [
      { id: 'v1', name: '仕事', accounts: [WORK], query: '' },
      { id: 'v2', name: '未読', accounts: [], query: 'is:unread' },
    ],
  })
  await app.openSettings()
  await app.page.locator('button[data-edit-view="v1"]').click()
  await expect(app.page.locator('#view-name')).toHaveValue('仕事')
  await expect(app.page.locator('#view-accounts').getByLabel(WORK)).toBeChecked()
  await app.page.locator('#view-name').fill('会社')
  await app.page.locator('#save-view').click()
  await expect(app.page.locator('#view-list .settings-row-name')).toHaveText(['会社', '未読'])

  await app.page.locator('button[data-delete-view="v2"]').click()
  await expect(app.page.locator('#view-list .settings-row-name')).toHaveText(['会社'])
  const saved = (await app.callsTo('save_views')) as { views: unknown[] }[]
  expect(saved.at(-1)!.views).toEqual([{ id: 'v1', name: '会社', accounts: [WORK], query: '' }])
})

test('deleting the view on screen goes back to すべて', async ({ launch }) => {
  const app = await launch({ views: [{ id: 'v1', name: '仕事', accounts: [], query: 'label:work' }] })
  await app.tab('仕事').click()
  await app.openSettings()
  await app.page.locator('button[data-delete-view="v1"]').click()
  await app.page.locator('#close-settings').click()
  await expect(app.tabs).toHaveText(['すべて'])
  await expect(app.tab('すべて')).toHaveAttribute('aria-selected', 'true')
  const calls = await app.callsTo('list_inbox')
  expect(calls.at(-1)).toEqual({ accounts: [], query: '' })
})

test('a failed save says why and keeps the views', async ({ launch }) => {
  const app = await launch({
    views: [{ id: 'v1', name: '仕事', accounts: [], query: '' }],
    failures: { save_views: { kind: 'internal', message: 'ビューを保存できませんでした: disk full' } },
  })
  await app.openSettings()
  await app.page.locator('button[data-delete-view="v1"]').click()
  await expect(app.page.locator('#view-error')).toHaveText('ビューを保存できませんでした: disk full')
  await expect(app.page.locator('#view-list .settings-row-name')).toHaveText(['仕事'])
})
