// The built app runs under the Content-Security-Policy in
// src-tauri/tauri.conf.json; the dev server every other spec loads from sends
// none. Anything the policy forbids therefore works in the whole suite and is
// missing only once the app is installed — which is how a checkbox lost the
// tick behind its `data:` URI. These cases run under the real policy.

import { expect, test } from './fixtures'

test('paints the checked boxes without tripping the policy', async ({ launch }) => {
  const app = await launch({ csp: true, state: { theme: 'dark' } })

  await app.gear.click()
  await expect(app.preferences).toBeVisible()
  const suggestions = app.page.locator('#pref-quick-suggestions')
  await suggestions.uncheck()
  await suggestions.check()

  // The tick is a background image, so a policy that blocks it leaves the box
  // filled and empty rather than failing outright.
  await expect(suggestions).toHaveCSS('background-image', /url\("data:image\/svg\+xml/)
  expect(app.cspViolations).toEqual([])
})

test('starts up without tripping the policy', async ({ launch }) => {
  const app = await launch({ csp: true })

  await app.typeInEditor('csp')
  await expect(app.chars).toHaveText('文字数: 3')
  expect(app.cspViolations).toEqual([])
})
