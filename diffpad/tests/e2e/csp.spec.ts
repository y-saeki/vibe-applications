// The built app runs under the Content-Security-Policy in
// src-tauri/tauri.conf.json; the dev server every other spec loads from sends
// none. Anything the policy forbids therefore works in the whole suite and is
// missing only once the app is installed. These cases run under the real
// policy.

import { expect, test } from './fixtures'

test('paints the masked marks without tripping the policy', async ({ launch }) => {
  const app = await launch({ csp: true, state: { showWhitespace: true, textA: 'a\tb' } })
  const maskOf = (selector: string, pseudo: string) =>
    app.page.evaluate(
      (target) => getComputedStyle(document.querySelector(target.selector)!, target.pseudo).maskImage,
      { selector, pseudo },
    )

  // The × that closes the panel, the arrow on the select and the one that
  // stands in for a tab are data: URIs painted through a mask, which the policy
  // covers under img-src. A policy that blocked one would leave the button, the
  // select or the tab simply blank rather than failing outright.
  expect(await maskOf('.cm-merge-a .cm-highlightTab', '')).toMatch(/url\("data:image\/svg\+xml/)

  await app.gear.click()
  await expect(app.preferences).toBeVisible()
  expect(await maskOf('#preferences-close', '::before')).toMatch(/url\("data:image\/svg\+xml/)

  await app.page.keyboard.press('Escape')
  // The arrow only exists where the list is ours; where the platform draws the
  // select, it draws the arrow too and there is nothing here to load.
  if (await app.listIsOurs()) {
    expect(await maskOf('#diff-mode-select', '::picker-icon')).toMatch(/url\("data:image\/svg\+xml/)
    await app.diffModeSelect.click()
  }
  expect(app.cspViolations).toEqual([])
})

test('starts up and diffs without tripping the policy', async ({ launch }) => {
  const app = await launch({ csp: true })

  await app.typeInEditor('a', 'csp')
  await expect(app.diffCount).toHaveText('差異 1 箇所')
  expect(app.cspViolations).toEqual([])
})
