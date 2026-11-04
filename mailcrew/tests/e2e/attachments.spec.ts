// The list of a card's attachments, opened from its toolbar, and opening and
// saving them.

import { attachment, card, expect, ME, test } from './fixtures'

const FILES = [
  attachment('1', '見積書.pdf', { mimeType: 'application/pdf', size: 123_456 }),
  attachment('2', 'setup.exe', { mimeType: 'application/x-msdownload', size: 512, openable: false }),
]

test('the toolbar button opens the list of attachments above itself and closes it again', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1, { attachments: FILES })]] })
  const button = app.card('t1').locator('.tool-attachments')
  const menu = app.page.locator('#attachment-menu')
  await expect(button).toHaveText('2')
  await expect(button).toHaveAccessibleName('添付ファイル 2 件')
  await expect(menu).toBeHidden()

  await button.click()
  await expect(menu).toBeVisible()
  await expect(button).toHaveAttribute('aria-expanded', 'true')
  const rows = menu.locator('.attachment-row')
  await expect(rows.locator('.attachment-name')).toHaveText(['見積書.pdf', 'setup.exe'])
  await expect(rows.locator('.attachment-size')).toHaveText(['121 KB', '512 B'])
  // A program can be saved but never opened from here.
  await expect(rows.nth(0).getByRole('button')).toHaveText(['開く', '保存'])
  await expect(rows.nth(1).getByRole('button')).toHaveText(['保存'])
  const menuBox = (await menu.boundingBox())!
  const buttonBox = (await button.boundingBox())!
  expect(menuBox.y + menuBox.height).toBeLessThanOrEqual(buttonBox.y)

  await button.click()
  await expect(menu).toBeHidden()
  await expect(button).toHaveAttribute('aria-expanded', 'false')
})

test('Escape and a click outside close the list', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1, { attachments: FILES })]] })
  const button = app.card('t1').locator('.tool-attachments')
  const menu = app.page.locator('#attachment-menu')
  await button.click()
  await expect(menu).toBeVisible()
  await app.page.keyboard.press('Escape')
  await expect(menu).toBeHidden()
  await expect(button).toBeFocused()

  await button.click()
  await app.card('t1').locator('.card-subject').click()
  await expect(menu).toBeHidden()
})

test('the attachment button sits after the other icons, and a card without attachments has none', async ({
  launch,
}) => {
  const app = await launch({ inboxes: [[card(1, { attachments: FILES }), card(2)]] })
  await expect(app.card('t1').locator('.tool-attachments')).toBeVisible()
  // Left to right as drawn: 完了 is first in the markup but sits at the far end.
  const order = await app.card('t1').locator('.card-toolbar button:visible').evaluateAll((buttons) =>
    buttons
      .map((button) => ({ action: (button as HTMLElement).dataset.action, left: button.getBoundingClientRect().left }))
      .sort((a, b) => a.left - b.left)
      .map((button) => button.action),
  )
  expect(order).toEqual(['tag', 'later', 'star', 'attachments', 'done'])
  await expect(app.frame('t2')).toBeAttached()
  await expect(app.card('t2').locator('.tool-attachments')).toBeHidden()
})

test('opening and saving go to the account and message the card shows', async ({ launch }) => {
  const app = await launch({
    accounts: [ME, 'work@example.com'],
    inboxes: [[card(1, { account: 'work@example.com', attachments: FILES })]],
  })
  const menu = app.page.locator('#attachment-menu')
  await app.card('t1').locator('.tool-attachments').click()
  await menu.getByRole('button', { name: '見積書.pdf を開く' }).click()
  await app.expectCalls('open_attachment', 1)
  expect(await app.callsTo('open_attachment')).toEqual([
    { account: 'work@example.com', messageId: 'm1', attachmentId: '1' },
  ])
  // Done with, the list closes.
  await expect(menu).toBeHidden()

  await app.card('t1').locator('.tool-attachments').click()
  await menu.getByRole('button', { name: 'setup.exe を保存' }).click()
  await expect(app.notice).toHaveText('ダウンロード フォルダに「setup.exe」を保存しました')
  expect(await app.callsTo('save_attachment')).toEqual([
    { account: 'work@example.com', messageId: 'm1', attachmentId: '2' },
  ])
  await expect(menu).toBeHidden()
})

test('an attachment that could not be opened says why and keeps the list open', async ({ launch }) => {
  const app = await launch({
    inboxes: [[card(1, { attachments: FILES })]],
    failures: { open_attachment: { kind: 'google', message: 'Gmail がエラーを返しました: Not Found' } },
  })
  const menu = app.page.locator('#attachment-menu')
  await app.card('t1').locator('.tool-attachments').click()
  const open = menu.getByRole('button', { name: '見積書.pdf を開く' })
  await open.click()
  await expect(app.notice).toHaveText('Gmail がエラーを返しました: Not Found')
  await expect(open).toBeEnabled()
  await expect(open).toHaveText('開く')
  await expect(menu).toBeVisible()
  await expect(app.cards).toHaveCount(1)
})

test('taking the card off the strip closes its list', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1, { attachments: FILES }), card(2)]] })
  const menu = app.page.locator('#attachment-menu')
  await app.card('t1').locator('.tool-attachments').click()
  await expect(menu).toBeVisible()
  await app.tool('t1', 'done').click()
  await expect(app.card('t1')).toHaveCount(0)
  await expect(menu).toBeHidden()
})
