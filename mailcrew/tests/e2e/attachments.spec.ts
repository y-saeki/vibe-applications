// The list of a card's attachments, and opening and saving them.

import { attachment, card, expect, ME, test } from './fixtures'

const FILES = [
  attachment('1', '見積書.pdf', { mimeType: 'application/pdf', size: 123_456 }),
  attachment('2', 'setup.exe', { mimeType: 'application/x-msdownload', size: 512, openable: false }),
]

test('the badge opens the list of attachments and closes it again', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1, { attachments: FILES })]] })
  const badge = app.card('t1').locator('.card-attachments')
  const list = app.card('t1').locator('.card-files')
  await expect(badge).toHaveText('2')
  await expect(list).toBeHidden()

  await badge.click()
  await expect(list).toBeVisible()
  await expect(badge).toHaveAttribute('aria-expanded', 'true')
  const rows = list.locator('.card-file')
  await expect(rows.locator('.card-file-name')).toHaveText(['見積書.pdf', 'setup.exe'])
  await expect(rows.locator('.card-file-size')).toHaveText(['121 KB', '512 B'])
  // A program can be saved but never opened from here.
  await expect(rows.nth(0).getByRole('button')).toHaveText(['開く', '保存'])
  await expect(rows.nth(1).getByRole('button')).toHaveText(['保存'])

  await badge.click()
  await expect(list).toBeHidden()
  await expect(badge).toHaveAttribute('aria-expanded', 'false')
})

test('a card without attachments has neither badge nor list', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1)]] })
  await expect(app.frame('t1')).toBeAttached()
  await expect(app.card('t1').locator('.card-attachments')).toBeHidden()
  await expect(app.card('t1').locator('.card-files')).toBeHidden()
})

test('opening and saving go to the account and message the card shows', async ({ launch }) => {
  const app = await launch({
    accounts: [ME, 'work@example.com'],
    inboxes: [[card(1, { account: 'work@example.com', attachments: FILES })]],
  })
  await app.card('t1').locator('.card-attachments').click()
  await app.card('t1').getByRole('button', { name: '見積書.pdf を開く' }).click()
  await app.expectCalls('open_attachment', 1)
  expect(await app.callsTo('open_attachment')).toEqual([
    { account: 'work@example.com', messageId: 'm1', attachmentId: '1' },
  ])

  await app.card('t1').getByRole('button', { name: 'setup.exe を保存' }).click()
  await expect(app.notice).toHaveText('ダウンロード フォルダに「setup.exe」を保存しました')
  expect(await app.callsTo('save_attachment')).toEqual([
    { account: 'work@example.com', messageId: 'm1', attachmentId: '2' },
  ])
})

test('an attachment that could not be opened says why', async ({ launch }) => {
  const app = await launch({
    inboxes: [[card(1, { attachments: FILES })]],
    failures: { open_attachment: { kind: 'google', message: 'Gmail がエラーを返しました: Not Found' } },
  })
  await app.card('t1').locator('.card-attachments').click()
  const open = app.card('t1').getByRole('button', { name: '見積書.pdf を開く' })
  await open.click()
  await expect(app.notice).toHaveText('Gmail がエラーを返しました: Not Found')
  await expect(open).toBeEnabled()
  // The card stays where it was.
  await expect(app.cards).toHaveCount(1)
})
