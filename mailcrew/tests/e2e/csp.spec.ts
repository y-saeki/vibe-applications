// The built app runs under the Content-Security-Policy in
// src-tauri/tauri.conf.json, and a message's frame inherits it on top of its
// own. These cases load the page under that policy, so a message's styles or
// an image the user allowed that the policy would block fail here rather than
// only in the installed app.

import { card, expect, test } from './fixtures'

const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
  'base64',
)

test('starts up without tripping the policy', async ({ launch }) => {
  const app = await launch({ csp: true, inboxes: [[card(1)]] })
  await expect(app.cards).toHaveCount(1)
  await expect(app.frame('t1').contentFrame().locator('p')).toHaveText('本文 1')
  expect(app.cspViolations).toEqual([])
})

test("a message's own styles apply inside the frame", async ({ launch }) => {
  const html = [
    '<style>.big { font-size: 30px }</style>',
    '<p class="big" style="color: rgb(200, 0, 0)">styled</p>',
    `<img id="inline" src="data:image/png;base64,${PNG.toString('base64')}">`,
  ].join('')
  const app = await launch({ csp: true, inboxes: [[card(1, { body: { kind: 'html', html, remoteImages: false } })]] })
  const frame = app.frame('t1').contentFrame()
  await expect(frame.locator('.big')).toHaveCSS('color', 'rgb(200, 0, 0)')
  await expect(frame.locator('.big')).toHaveCSS('font-size', '30px')
  await expect
    .poll(() => frame.locator('#inline').evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth > 0))
    .toBe(true)
})

test('an image the user allowed gets through the app policy too', async ({ launch, page }) => {
  await page.route('https://images.example.test/**', (route) =>
    route.fulfill({ status: 200, contentType: 'image/png', body: PNG }),
  )
  const html = '<img id="remote" src="https://images.example.test/a.png">'
  const app = await launch({ csp: true, inboxes: [[card(1, { body: { kind: 'html', html, remoteImages: true } })]] })
  await app.card('t1').getByRole('button', { name: '表示する' }).click()
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
