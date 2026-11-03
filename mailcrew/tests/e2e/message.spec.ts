// How a message's body is shown, and what it is kept from doing. The bodies
// here skip the Rust sanitiser on purpose: these cases are about the second
// layer, the frame, holding on its own.

import type { Page } from '@playwright/test'

import { card, cards, expect, test } from './fixtures'

const IMAGE_HOST = 'https://images.example.test'
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
  'base64',
)

/** Counts the requests that reach the image host. */
async function countImageRequests(page: Page): Promise<() => number> {
  let count = 0
  await page.route(`${IMAGE_HOST}/**`, (route) => {
    count += 1
    return route.fulfill({ status: 200, contentType: 'image/png', body: PNG })
  })
  return () => count
}

test('shows HTML in a frame that may not run scripts', async ({ launch }) => {
  const html = [
    '<p id="target">static</p>',
    '<script>document.getElementById("target").textContent = "ran"; parent.__ran = true</script>',
    '<img src="data:image/png;base64,broken" onerror="parent.__ran = true">',
  ].join('')
  const app = await launch({ inboxes: [[card(1, { body: { kind: 'html', html, remoteImages: false } })]] })
  const frame = app.frame('t1')
  await expect(frame).toHaveAttribute('sandbox', 'allow-same-origin')
  await expect(frame).toHaveAttribute('referrerpolicy', 'no-referrer')
  await expect(frame.contentFrame().locator('#target')).toHaveText('static')
  expect(await app.page.evaluate(() => (window as unknown as { __ran?: boolean }).__ran)).toBeUndefined()
})

test('loads no image from the network until asked', async ({ launch }) => {
  const html = [
    `<img id="remote" src="${IMAGE_HOST}/pixel.png" width="1" height="1">`,
    `<div style="background: url(${IMAGE_HOST}/bg.png); width: 10px; height: 10px"></div>`,
  ].join('')
  const app = await launch({ inboxes: [[card(1, { body: { kind: 'html', html, remoteImages: true } })]] })
  const requests = await countImageRequests(app.page)
  // Routed after launch, so the page is loaded again under the route.
  await app.page.reload()
  const images = app.card('t1').locator('.card-images')
  await expect(images).toBeVisible()
  await expect(app.frame('t1').contentFrame().locator('#remote')).toBeAttached()
  // Give a load that should not happen the time to happen.
  await app.page.waitForTimeout(500)
  expect(requests()).toBe(0)

  await images.getByRole('button', { name: '表示する' }).click()
  await expect(images).toBeHidden()
  await expect.poll(requests).toBeGreaterThanOrEqual(2)
  await expect
    .poll(() =>
      app
        .frame('t1')
        .contentFrame()
        .locator('#remote')
        .evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth > 0),
    )
    .toBe(true)
})

test('a message without remote images offers nothing to show', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1)]] })
  await expect(app.frame('t1')).toBeAttached()
  await expect(app.card('t1').locator('.card-images')).toBeHidden()
})

// The page catches a click inside the frame and hands the link to the Rust
// side. WebKit runs no listener inside a frame that may not run scripts, so
// there the click goes ahead as a navigation of the frame, which WKWebView
// hands to src-tauri/src/links.rs to open outside instead. Each engine is held
// to its own path: Chromium must never navigate, WebKit must navigate to
// exactly the link (and nowhere the guard would not see).
test('a link leaves the frame by the path its engine takes', async ({ launch, browserName }) => {
  const html = '<p><a id="link" href="https://example.com/news?id=1">記事</a></p>'
  const app = await launch({ inboxes: [[card(1, { body: { kind: 'html', html, remoteImages: false } })]] })
  const navigations: string[] = []
  await app.page.route('https://example.com/**', (route) => {
    if (route.request().isNavigationRequest()) navigations.push(route.request().url())
    return route.abort()
  })
  await app.frame('t1').contentFrame().locator('#link').click()

  if (browserName === 'webkit') {
    await expect.poll(() => navigations).toEqual(['https://example.com/news?id=1'])
    expect(await app.callsTo('open_link')).toEqual([])
  } else {
    await app.expectCalls('open_link', 1)
    expect(await app.callsTo('open_link')).toEqual([{ url: 'https://example.com/news?id=1' }])
    await expect(app.frame('t1').contentFrame().locator('#link')).toBeVisible()
    expect(navigations).toEqual([])
  }
})

test('plain text is shown as text, with its addresses made into links', async ({ launch }) => {
  const text = 'こんにちは <b>太字ではない</b>\n詳しくは https://example.com/a?b=1 を見てください。'
  const app = await launch({ inboxes: [[card(1, { body: { kind: 'text', text } })]] })
  const body = app.card('t1').locator('.message-text')
  await expect(body).toContainText('<b>太字ではない</b>')
  await expect(app.frame('t1')).toHaveCount(0)
  await body.getByRole('link', { name: 'https://example.com/a?b=1' }).click()
  expect(await app.callsTo('open_link')).toEqual([{ url: 'https://example.com/a?b=1' }])
})

test('an empty body says so', async ({ launch }) => {
  const app = await launch({ inboxes: [[card(1, { body: { kind: 'empty' } })]] })
  await expect(app.card('t1').locator('.message-empty')).toHaveText('本文はありません')
})

test('a message laid out wider than the card is scaled to fit', async ({ launch }) => {
  const html = '<table width="900" style="width: 900px"><tr><td>wide newsletter</td></tr></table>'
  const narrow = '<p>narrow</p>'
  const app = await launch({
    inboxes: [
      [
        card(1, { body: { kind: 'html', html, remoteImages: false } }),
        card(2, { body: { kind: 'html', html: narrow, remoteImages: false } }),
      ],
    ],
  })
  const zoomOf = (id: string) =>
    app
      .frame(id)
      .contentFrame()
      .locator('html')
      .evaluate((root) => root.style.getPropertyValue('zoom'))
  await expect.poll(() => zoomOf('t1')).toMatch(/^0\.\d+$/)
  await expect.poll(() => app.frame('t2').contentFrame().locator('p').textContent()).toBe('narrow')
  expect(await zoomOf('t2')).toBe('')
})

test('bodies far off to the right wait until the strip comes near them', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(30)] })
  await expect(app.frame('t1')).toBeAttached()
  await expect(app.frame('t30')).toHaveCount(0)
  await expect(app.card('t30').locator('.card-snippet')).toHaveText('スニペット 30')
  await app.card('t30').scrollIntoViewIfNeeded()
  await expect(app.frame('t30')).toBeAttached()
})
