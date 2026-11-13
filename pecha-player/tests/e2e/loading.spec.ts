import { expect, test } from './fixtures'

const DECK = 'https://docs.google.com/presentation/d/1AbCdEfGhIjKlMnOpQrStUvWxYz0123456789_-abcde/edit'

test('opens on the loader, with Start unavailable', async ({ launch }) => {
  const app = await launch()
  await expect(app.loader).toBeVisible()
  await expect(app.primary).toBeDisabled()
  await expect(app.page.locator('#close-loader')).toBeHidden()
})

test('a local PDF shows its cover and waits for Start', async ({ launch }) => {
  const app = await launch()
  await app.loadPdf(21)
  await app.expectPhase('ready')
  await app.expectPage(1)
  await expect(app.loader).toBeHidden()
  await expect(app.primary).toBeEnabled()
  await expect(app.primary).toHaveText('スタート')
  await expect(app.deckInfo).toHaveText('talk.pdf · 表紙 + 20 枚')
  await expect(app.counter).toHaveText('0 / 20')
  await expect(app.clock).toHaveText('0:00')
})

test('a deck short of 20 slides says so', async ({ launch }) => {
  const app = await launch()
  await app.loadPdf(9)
  await app.expectPhase('ready')
  await expect(app.deckInfo).toHaveText('talk.pdf · 表紙 + 8 枚（20 枚に足りません）')
  await expect(app.deckInfo).toHaveClass(/warning/)
  await expect(app.counter).toHaveText('0 / 8')
})

test('pages past the 21st are not played', async ({ launch }) => {
  const app = await launch()
  await app.loadPdf(30)
  await expect(app.deckInfo).toHaveText('talk.pdf · 表紙 + 20 枚')
})

test('a file that is not a PDF is refused', async ({ launch }) => {
  const app = await launch()
  await app.page.locator('#file-input').setInputFiles({
    name: 'talk.pdf',
    mimeType: 'application/pdf',
    buffer: Buffer.from('<html>not a pdf</html>'),
  })
  await expect(app.error).toHaveText('PDF ファイルではありません。')
  await app.expectPhase('empty')
})

test('a cover alone is not enough', async ({ launch }) => {
  const app = await launch()
  await app.loadPdf(1)
  await expect(app.error).toHaveText('表紙のほかに 1 ページ以上必要です。')
  await app.expectPhase('empty')
})

test('a Google Slides link loads the deck', async ({ launch }) => {
  const app = await launch({ google: { [DECK]: [{ pages: 21 }] } })
  await app.loadUrl(DECK)
  await app.expectPhase('ready')
  await app.expectPage(1)
  await expect(app.deckInfo).toHaveText('Google スライド · 表紙 + 20 枚')
  expect((await app.calls()).filter((c) => c.cmd === 'load_google_slides')).toEqual([
    { cmd: 'load_google_slides', args: { url: DECK } },
  ])
})

test('a link that is not a deck is explained', async ({ launch }) => {
  const app = await launch()
  await app.loadUrl('https://example.com/slides')
  await expect(app.error).toContainText('Google スライドの URL ではありません')
})

test('a private deck asks for a Google sign-in, then loads by itself', async ({ launch }) => {
  const app = await launch({ google: { [DECK]: [{ error: { kind: 'signInRequired' } }, { pages: 21 }] } })
  await app.loadUrl(DECK)
  await expect(app.signIn).toBeVisible()
  await app.signInButton.click()
  await expect(app.signInButton).toBeDisabled()
  expect((await app.calls()).some((c) => c.cmd === 'open_google_sign_in')).toBe(true)

  await app.emit('google-sign-in', 'done')
  await app.expectPhase('ready')
  await expect(app.deckInfo).toHaveText('Google スライド · 表紙 + 20 枚')
})

test('closing the sign-in window early lets the user try again', async ({ launch }) => {
  const app = await launch({ google: { [DECK]: [{ error: { kind: 'signInRequired' } }] } })
  await app.loadUrl(DECK)
  await app.signInButton.click()
  await app.emit('google-sign-in', 'closed')
  await expect(app.page.locator('#sign-in-text')).toHaveText('ログインが終わる前にウィンドウが閉じられました。')
  await expect(app.signInButton).toBeEnabled()
})

test('an account without access is told so', async ({ launch }) => {
  const app = await launch({ google: { [DECK]: [{ error: { kind: 'forbidden' } }] } })
  await app.loadUrl(DECK)
  await expect(app.error).toHaveText('このアカウントには、スライドを見る権限がありません。')
})

test('loading another deck keeps the current one until the new one is in', async ({ launch }) => {
  const app = await launch()
  await app.loadPdf(21)
  await app.expectPhase('ready')
  await app.loadButton.click()
  await expect(app.loader).toBeVisible()
  await app.loadPdf(1)
  await expect(app.error).toBeVisible()
  await app.page.locator('#close-loader').click()
  await expect(app.loader).toBeHidden()
  await app.expectPhase('ready')
  await expect(app.deckInfo).toHaveText('talk.pdf · 表紙 + 20 枚')
})
