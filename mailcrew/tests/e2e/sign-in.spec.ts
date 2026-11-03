// Who is signed in decides the first screen; signing in and out move between them.

import { card, expect, test } from './fixtures'

test('asks for the OAuth client when nobody is signed in', async ({ launch }) => {
  const app = await launch({ email: null })
  await expect(app.setup).toBeVisible()
  await expect(app.page.locator('#inbox')).toBeHidden()
  expect(await app.callsTo('list_inbox')).toEqual([])
})

test('signs in with the client and opens the inbox', async ({ launch }) => {
  const app = await launch({ email: null, signInEmail: 'new@example.com', inboxes: [[card(1)]] })
  await app.page.locator('#client-id').fill('id.apps.googleusercontent.com')
  await app.page.locator('#client-secret').fill('secret')
  await app.page.locator('#sign-in').click()

  await expect(app.cards).toHaveCount(1)
  await expect(app.page.locator('#account')).toHaveText('new@example.com')
  expect(await app.callsTo('sign_in')).toEqual([{ clientId: 'id.apps.googleusercontent.com', clientSecret: 'secret' }])
  await expect(app.setup).toBeHidden()
})

test('a failed sign-in says why and stays on the form', async ({ launch }) => {
  const app = await launch({
    email: null,
    failures: { sign_in: { kind: 'google', message: 'Google がログインを許可しませんでした (access_denied)' } },
  })
  await app.page.locator('#client-id').fill('id')
  await app.page.locator('#client-secret').fill('secret')
  await app.page.locator('#sign-in').click()
  await expect(app.page.locator('#setup-error')).toHaveText('Google がログインを許可しませんでした (access_denied)')
  await expect(app.page.locator('#sign-in')).toBeEnabled()
})

test('a cancelled sign-in goes back to the form without an error', async ({ launch }) => {
  const app = await launch({
    email: null,
    failures: { sign_in: { kind: 'cancelled', message: 'ログインを中止しました' } },
  })
  await app.page.locator('#client-id').fill('id')
  await app.page.locator('#client-secret').fill('secret')
  await app.page.locator('#sign-in').click()
  await expect(app.page.locator('#sign-in')).toBeEnabled()
  await expect(app.page.locator('#setup-error')).toBeHidden()
})

test('opens the inbox straight away when an account is stored', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1), card(2)]] })
  await expect(app.cards).toHaveCount(2)
  await expect(app.setup).toBeHidden()
  await expect(app.page.locator('#account')).toHaveText('me@example.com')
})

test('signing out goes back to the form', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1)]] })
  await expect(app.cards).toHaveCount(1)
  await app.page.locator('#sign-out').click()
  await expect(app.setup).toBeVisible()
  await expect(app.cards).toHaveCount(0)
  await app.expectCalls('sign_out', 1)
})

test('an expired login sends the user back to sign in', async ({ launch }) => {
  const app = await launch({
    failures: { list_inbox: { kind: 'signedOut', message: 'Google のログインが切れました。もう一度ログインしてください' } },
  })
  await expect(app.setup).toBeVisible()
  await expect(app.page.locator('#setup-error')).toHaveText('Google のログインが切れました。もう一度ログインしてください')
})
