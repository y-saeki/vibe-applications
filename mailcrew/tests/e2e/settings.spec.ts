// The settings screen: adding and removing accounts, and the senders whose
// images are always shown.

import { card, expect, ME, test } from './fixtures'

test('lists the signed-in accounts and goes back to the inbox', async ({ launch }) => {
  const app = await launch({ accounts: [ME, 'work@example.com'], inboxes: [[card(1)]] })
  await expect(app.cards).toHaveCount(1)
  await app.openSettings()
  await expect(app.page.locator('#inbox')).toBeHidden()
  await expect(app.page.locator('#account-list .settings-row-name')).toHaveText([ME, 'work@example.com'])
  await app.page.locator('#close-settings').click()
  await expect(app.settings).toBeHidden()
  await expect(app.cards).toHaveCount(1)
  await app.expectCalls('list_inbox', 2)
})

test('adds an account with the client already stored', async ({ launch }) => {
  const app = await launch({ signInEmail: 'work@example.com', inboxes: [[card(1)]] })
  await app.openSettings()
  await app.page.locator('#add-account').click()
  await expect(app.page.locator('#account-list .settings-row-name')).toHaveText([ME, 'work@example.com'])
  expect(await app.callsTo('sign_in')).toEqual([{ clientId: null, clientSecret: null }])
  await expect(app.page.locator('#add-account')).toBeEnabled()
})

test('adds an account with another client', async ({ launch }) => {
  const app = await launch({ signInEmail: 'work@example.com' })
  await app.openSettings()
  await app.page.locator('#other-client summary').click()
  await app.page.locator('#account-client-id').fill('other.apps.googleusercontent.com')
  await app.page.locator('#account-client-secret').fill('other-secret')
  await app.page.locator('#add-account').click()
  await expect(app.page.locator('#account-list .settings-row-name')).toHaveCount(2)
  expect(await app.callsTo('sign_in')).toEqual([
    { clientId: 'other.apps.googleusercontent.com', clientSecret: 'other-secret' },
  ])
  await expect(app.page.locator('#account-client-secret')).toHaveValue('')
})

test('a failed sign-in says why and keeps the accounts', async ({ launch }) => {
  const app = await launch({
    failures: { sign_in: { kind: 'google', message: 'Google がログインを許可しませんでした (access_denied)' } },
  })
  await app.openSettings()
  await app.page.locator('#add-account').click()
  await expect(app.page.locator('#account-error')).toHaveText('Google がログインを許可しませんでした (access_denied)')
  await expect(app.page.locator('#account-list .settings-row-name')).toHaveText([ME])
})

test('a cancelled sign-in says nothing', async ({ launch }) => {
  const app = await launch({ failures: { sign_in: { kind: 'cancelled', message: 'ログインを中止しました' } } })
  await app.openSettings()
  await app.page.locator('#add-account').click()
  await expect(app.page.locator('#add-account')).toBeEnabled()
  await expect(app.page.locator('#account-error')).toBeHidden()
})

test('removes one account and keeps the rest', async ({ launch }) => {
  const app = await launch({ accounts: [ME, 'work@example.com'] })
  await app.openSettings()
  await app.page.locator('button[data-remove-account="work@example.com"]').click()
  await expect(app.page.locator('#account-list .settings-row-name')).toHaveText([ME])
  expect(await app.callsTo('sign_out')).toEqual([{ account: 'work@example.com' }])
  await expect(app.settings).toBeVisible()
})

test('removing the last account goes back to the first screen', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1)]] })
  await app.openSettings()
  await app.page.locator(`button[data-remove-account="${ME}"]`).click()
  await expect(app.setup).toBeVisible()
  await expect(app.settings).toBeHidden()
  await expect(app.cards).toHaveCount(0)
})

test('lists the senders whose images are always shown, and removes one', async ({ launch }) => {
  const app = await launch({ imageSenders: ['news@example.com', 'shop@example.jp'] })
  await app.openSettings()
  const senders = app.page.locator('#image-sender-list .settings-row-name')
  await expect(senders).toHaveText(['news@example.com', 'shop@example.jp'])
  await app.page.locator('button[data-remove-sender="news@example.com"]').click()
  await expect(senders).toHaveText(['shop@example.jp'])
  expect(await app.callsTo('save_image_senders')).toEqual([{ senders: ['shop@example.jp'] }])
  await app.page.locator('button[data-remove-sender="shop@example.jp"]').click()
  await expect(app.page.locator('#image-sender-list')).toHaveText('画像を常に表示する差出人はいません')
})

test('a sender that could not be removed says why and stays', async ({ launch }) => {
  const app = await launch({
    imageSenders: ['news@example.com'],
    failures: { save_image_senders: { kind: 'internal', message: '保存できませんでした' } },
  })
  await app.openSettings()
  await app.page.locator('button[data-remove-sender="news@example.com"]').click()
  await expect(app.page.locator('#image-sender-error')).toHaveText('保存できませんでした')
  await expect(app.page.locator('#image-sender-list .settings-row-name')).toHaveText(['news@example.com'])
})
