// The built app runs under the Content-Security-Policy in
// src-tauri/tauri.conf.json; the dev server every other spec loads from sends
// none. Anything the policy forbids therefore works in the whole suite and is
// missing only once the app is installed — which is how a checkbox lost the
// tick behind its `data:` URI. These cases run under the real policy.

import { expect, test } from './fixtures'

test('paints the toggle in both states without tripping the policy', async ({ launch }) => {
  const app = await launch({ csp: true, state: { theme: 'dark' } })

  // The toggle in the preferences panel carries a mark in both states, and it
  // sits on the knob rather than on the control itself. Each mark is a
  // background image, so a policy that blocks it leaves the knob bare rather
  // than failing outright.
  await app.gear.click()
  await expect(app.preferences).toBeVisible()
  const knob = () =>
    app.page.evaluate(
      () => getComputedStyle(document.querySelector('#pref-quick-suggestions')!, '::before').backgroundImage,
    )
  const suggestions = app.page.locator('#pref-quick-suggestions')
  await suggestions.uncheck()
  expect(await knob()).toMatch(/url\("data:image\/svg\+xml/)
  await suggestions.check()
  expect(await knob()).toMatch(/url\("data:image\/svg\+xml/)

  expect(app.cspViolations).toEqual([])
})

test('paints the masked marks without tripping the policy', async ({ launch }) => {
  const app = await launch({ csp: true, state: { showWhitespace: true, text: 'a\tb' } })
  const maskOf = (selector: string, pseudo: string) =>
    app.page.evaluate(
      (target) => getComputedStyle(document.querySelector(target.selector)!, target.pseudo).maskImage,
      { selector, pseudo },
    )

  // The × that closes a panel, the arrow on a select and the one that stands in
  // for a tab are data: URIs painted through a mask, which the policy covers
  // under img-src. A policy that blocked one would leave the button, the select
  // or the tab simply blank rather than failing outright.
  expect(await maskOf('.cm-highlightTab', '')).toMatch(/url\("data:image\/svg\+xml/)

  await app.gear.click()
  await expect(app.preferences).toBeVisible()
  expect(await maskOf('#preferences-close', '::before')).toMatch(/url\("data:image\/svg\+xml/)

  await app.page.keyboard.press('Escape')
  // The arrow only exists where the list is ours; where the platform draws the
  // select, it draws the arrow too and there is nothing here to load.
  if (await app.listIsOurs()) {
    expect(await maskOf('#language-select', '::picker-icon')).toMatch(/url\("data:image\/svg\+xml/)
    await app.languageSelect.click()
  }

  // The search panel is the same story four times over: the previous and next
  // buttons and the two option toggles all collapse their words and wear a
  // masked mark instead.
  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()
  for (const selector of [
    '.cm-search button[name="prev"]',
    '.cm-search button[name="next"]',
    '.cm-search label:nth-of-type(1)',
    '.cm-search label:nth-of-type(2)',
  ]) {
    expect(await maskOf(selector, '::before')).toMatch(/url\("data:image\/svg\+xml/)
  }
  expect(app.cspViolations).toEqual([])
})

test('starts up without tripping the policy', async ({ launch }) => {
  const app = await launch({ csp: true })

  await app.typeInEditor('csp')
  await expect(app.chars).toHaveText('3 文字')
  expect(app.cspViolations).toEqual([])
})
