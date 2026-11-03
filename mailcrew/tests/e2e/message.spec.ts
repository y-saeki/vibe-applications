// How a message's body is shown, and what it is kept from doing. The bodies
// here skip the Rust sanitiser on purpose: these cases are about the second
// layer, the frame, holding on its own.

import type { Page } from '@playwright/test'

import { card, cards, expect, ME, test } from './fixtures'

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

/** Records the path of every request that reaches the image host. */
async function recordImageRequests(page: Page): Promise<string[]> {
  const paths: string[] = []
  await page.route(`${IMAGE_HOST}/**`, (route) => {
    paths.push(new URL(route.request().url()).pathname)
    return route.fulfill({ status: 200, contentType: 'image/png', body: PNG })
  })
  return paths
}

/** A card from `email` whose message shows `${IMAGE_HOST}/<n>.png`. */
function cardWithImage(n: number, email: string) {
  const html = `<img src="${IMAGE_HOST}/${n}.png" width="1" height="1">`
  return card(n, { from: { name: email, email }, body: { kind: 'html', html, remoteImages: true } })
}

test('a sender whose images are always shown has them shown without asking', async ({ launch }) => {
  const app = await launch({
    imageSenders: ['news@example.com'],
    inboxes: [[cardWithImage(1, 'News@Example.com'), cardWithImage(2, 'other@example.com')]],
  })
  const requests = await recordImageRequests(app.page)
  await app.page.reload()
  await expect(app.frame('t2')).toBeAttached()
  await expect.poll(() => requests).toContain('/1.png')
  await expect(app.card('t1').locator('.card-images')).toBeHidden()
  await expect(app.card('t2').locator('.card-images')).toBeVisible()
  await app.page.waitForTimeout(500)
  expect(requests).not.toContain('/2.png')
})

test('この差出人は常に表示 remembers the sender and shows their images in every card', async ({ launch }) => {
  const app = await launch({
    inboxes: [[cardWithImage(1, 'news@example.com'), cardWithImage(2, 'news@example.com'), cardWithImage(3, 'other@example.com')]],
  })
  const requests = await recordImageRequests(app.page)
  await app.page.reload()
  await expect(app.frame('t3')).toBeAttached()
  await app.page.waitForTimeout(500)
  expect(requests).toEqual([])

  await app.card('t1').getByRole('button', { name: 'この差出人は常に表示' }).click()
  expect(await app.callsTo('save_image_senders')).toEqual([{ senders: ['news@example.com'] }])
  await expect.poll(() => [...requests].sort()).toEqual(['/1.png', '/2.png'])
  await expect(app.card('t1').locator('.card-images')).toBeHidden()
  await expect(app.card('t2').locator('.card-images')).toBeHidden()
  await expect(app.card('t3').locator('.card-images')).toBeVisible()

  // The choice outlives the strip: loaded again, the sender's cards no longer ask.
  await app.page.locator('#reload').click()
  await app.expectCalls('list_inbox', 2)
  await expect(app.card('t1').locator('.card-images')).toBeHidden()
  await expect(app.card('t3').locator('.card-images')).toBeVisible()
})

test('a sender that could not be remembered says why and keeps asking', async ({ launch }) => {
  const app = await launch({
    inboxes: [[cardWithImage(1, 'news@example.com')]],
    failures: { save_image_senders: { kind: 'internal', message: '保存できませんでした' } },
  })
  const requests = await recordImageRequests(app.page)
  await app.page.reload()
  await app.card('t1').getByRole('button', { name: 'この差出人は常に表示' }).click()
  await expect(app.notice).toContainText('保存できませんでした')
  await expect(app.card('t1').locator('.card-images')).toBeVisible()
  expect(requests).toEqual([])
})

// The page catches a click inside the frame and hands the link to the Rust
// side. WebKit runs no listener inside a frame that may not run scripts, so
// there the click goes ahead as a navigation of the frame, which WKWebView
// hands to src-tauri/src/links.rs to open outside instead. Each engine is held
// to its own path: Chromium must never navigate, WebKit must navigate to
// exactly the link (and nowhere the guard would not see).
//
// The page runs under the app's policy: WebKit checks `frame-src` before it
// asks the guard, so a policy that refused the link would leave the click
// doing nothing at all.
test('a link leaves the frame by the path its engine takes', async ({ launch, browserName }) => {
  const html = '<p><a id="link" href="https://example.com/news?id=1">記事</a></p>'
  const app = await launch({ csp: true, inboxes: [[card(1, { body: { kind: 'html', html, remoteImages: false } })]] })
  const navigations: string[] = []
  await app.page.route('https://example.com/**', (route) => {
    if (route.request().isNavigationRequest()) navigations.push(route.request().url())
    return route.abort()
  })
  await app.frame('t1').contentFrame().locator('#link').click()

  if (browserName === 'webkit') {
    await expect.poll(() => navigations).toEqual(['https://example.com/news?id=1'])
    expect(await app.callsTo('open_link')).toEqual([])
    expect(app.cspViolations).toEqual([])
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

test('a long message scrolls in the card, and its end can be reached', async ({ launch }) => {
  const rows = Array.from({ length: 60 }, (_, i) => `<tr><td>行 ${i}</td></tr>`).join('')
  const html = `<table width="1200" style="width: 1200px">${rows}</table><p id="end">終わり</p>`
  const app = await launch({ inboxes: [[card(1, { body: { kind: 'html', html, remoteImages: false } })]] })
  const body = app.card('t1').locator('.card-body')
  const end = app.frame('t1').contentFrame().locator('#end')
  await expect(end).toBeAttached()
  // The frame is as tall as the message, so the card body is what scrolls.
  await expect
    .poll(() => body.evaluate((element) => element.scrollHeight > element.clientHeight))
    .toBe(true)
  await body.evaluate((element) => element.scrollTo(0, element.scrollHeight))
  await expect(end).toBeInViewport()
})

test('nothing inside a card scrolls sideways', async ({ launch }) => {
  const wide = '<table width="1400" style="width: 1400px"><tr><td>wide</td></tr></table><pre>' + 'x'.repeat(400) + '</pre>'
  const longText = 'https://example.com/' + 'a'.repeat(300) + '\n' + 'あ'.repeat(500)
  const app = await launch({
    inboxes: [
      [
        card(1, { body: { kind: 'html', html: wide, remoteImages: false } }),
        card(2, { body: { kind: 'text', text: longText } }),
      ],
    ],
  })
  await expect(app.frame('t1').contentFrame().locator('table')).toBeAttached()
  await expect(app.card('t2').locator('.message-text')).toBeAttached()
  for (const id of ['t1', 't2']) {
    const body = app.card(id).locator('.card-body')
    expect(await body.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true)
    expect(await body.evaluate((element) => getComputedStyle(element).overflowX)).toBe('hidden')
  }
  // The frame's own document hides its overflow too, and is scaled to fit.
  const root = app.frame('t1').contentFrame().locator('html')
  expect(await root.evaluate((element) => getComputedStyle(element).overflowX)).toBe('hidden')
  await expect
    .poll(() =>
      root.evaluate((element) => element.getBoundingClientRect().width <= (window.frameElement as HTMLElement).clientWidth + 1),
    )
    .toBe(true)
})

test('the scrollbar shows only while the body is scrolled', async ({ launch }) => {
  const app = await launch({
    inboxes: [
      [
        card(1, { body: { kind: 'text', text: '行\n'.repeat(300) } }),
        card(2, { body: { kind: 'text', text: '短い本文' } }),
      ],
    ],
  })
  const body = app.card('t1').locator('.card-body')
  const bar = app.card('t1').locator('.scrollbar')
  await expect(app.card('t1').locator('.message-text')).toBeAttached()
  // The platform's own bar takes no room.
  expect(await body.evaluate((element: HTMLElement) => element.offsetWidth - element.clientWidth)).toBe(0)
  await expect(bar).not.toHaveAttribute('data-shown')
  await body.evaluate((element) => element.scrollBy(0, 200))
  await expect(bar).toHaveAttribute('data-shown', '')
  await expect(bar).not.toHaveAttribute('data-shown', { timeout: 3000 })
  // A body with nothing to scroll has no bar at all.
  await expect(app.card('t2').locator('.scrollbar')).toBeHidden()
})

test('bodies far off to the right are neither fetched nor shown until the strip comes near them', async ({ launch }) => {
  const app = await launch({ inboxes: [cards(30)] })
  await expect(app.frame('t1')).toBeAttached()
  await expect(app.frame('t30')).toHaveCount(0)
  await expect(app.card('t30').locator('.card-snippet')).toHaveText('スニペット 30')
  const fetched = async () => ((await app.callsTo('get_message')) as { messageId: string }[]).map((call) => call.messageId)
  expect(await fetched()).toContain('m1')
  expect(await fetched()).not.toContain('m30')
  await app.card('t30').scrollIntoViewIfNeeded()
  await expect(app.frame('t30')).toBeAttached()
  expect(await fetched()).toContain('m30')
})

test('a body is fetched from the account its card is in', async ({ launch }) => {
  const app = await launch({
    accounts: [ME, 'work@example.com'],
    inboxes: [[card(1, { account: 'work@example.com', attachments: ['a.pdf'] })]],
  })
  await expect(app.frame('t1')).toBeAttached()
  expect(await app.callsTo('get_message')).toEqual([{ account: 'work@example.com', messageId: 'm1' }])
  await expect(app.card('t1').locator('.card-attachments')).toHaveText('1')
})

test('a body that could not be fetched leaves the snippet and says why', async ({ launch }) => {
  const app = await launch({
    inboxes: [[card(1)]],
    failures: { get_message: { kind: 'google', message: 'Gmail の利用上限に達しました。1 分ほど待ってから再読み込みしてください' } },
  })
  await expect(app.notice).toHaveText('Gmail の利用上限に達しました。1 分ほど待ってから再読み込みしてください')
  await expect(app.card('t1').locator('.card-snippet')).toHaveText('スニペット 1')
  await expect(app.frame('t1')).toHaveCount(0)
})
