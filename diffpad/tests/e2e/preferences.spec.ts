import { expect, test } from './fixtures'

test('opens from the gear button and closes with Escape', async ({ launch }) => {
  const app = await launch()

  await expect(app.preferences).toBeHidden()
  await app.gear.click()
  await expect(app.preferences).toBeVisible()

  await app.page.keyboard.press('Escape')
  await expect(app.preferences).toBeHidden()
  // Closing hands the keyboard back, so typing goes into a pane again.
  await expect(app.editorA).toBeFocused()
})

test('hands the keyboard back to the pane that had it', async ({ launch }) => {
  const app = await launch()

  await app.editorB.click()
  await expect(app.editorB).toBeFocused()
  await app.gear.click()
  await app.page.keyboard.press('Escape')
  await expect(app.editorB).toBeFocused()
})

test('closes when the backdrop is clicked', async ({ launch }) => {
  const app = await launch()

  await app.gear.click()
  await expect(app.preferences).toBeVisible()
  await app.preferences.click({ position: { x: 4, y: 4 } })
  await expect(app.preferences).toBeHidden()
})

test('shows the version, set apart from the settings above it', async ({ launch }) => {
  const app = await launch({ version: '9.8.7' })

  await app.gear.click()
  const version = app.page.locator('#pref-version')
  await expect(version).toHaveText('diffpad 9.8.7')
  // Not a setting, so it is deliberately outside the label column: bottom
  // right, in figures whose width does not move as the number changes.
  await expect(version).toHaveCSS('text-align', 'right')
  await expect(version).toHaveCSS('font-family', /ui-monospace|SFMono-Regular|Menlo|Consolas|monospace/)
})

test('gathers the settings into the two groups, in that order', async ({ launch }) => {
  const app = await launch()

  await app.gear.click()
  await expect(app.page.locator('#preferences legend')).toHaveText(['編集', '表示'])

  // The order in the panel is the order the keyboard walks them in, so one
  // check covers both.
  const ids = [
    '#pref-tab-size',
    '#pref-theme',
    '#pref-font-family',
    '#pref-font-weight',
    '#pref-font-size',
    '#pref-show-whitespace',
  ]
  await app.page.locator('#pref-tab-size').focus()
  for (const id of ids) {
    await expect(app.page.locator(id)).toBeFocused()
    await app.page.keyboard.press('Tab')
  }

  // Which group a row belongs to is the point of the split.
  const groupOf = (id: string) =>
    app.page.evaluate(
      (selector) => document.querySelector(selector)!.closest('.pref-group')!.querySelector('legend')!.textContent,
      id,
    )
  expect(await groupOf('#pref-tab-size')).toBe('編集')
  expect(await groupOf('#pref-theme')).toBe('表示')
  expect(await groupOf('#pref-show-whitespace')).toBe('表示')

  // One rule between the two, and it hangs off the group above: a fieldset's
  // block-start border is the one its legend notches and sits on, so a
  // border-top would be drawn straight through the heading.
  const groups = app.page.locator('#preferences .pref-group')
  await expect(groups).toHaveCount(2)
  await expect(groups.first()).toHaveCSS('border-bottom-width', '1px')
  await expect(groups.first()).toHaveCSS('border-top-width', '0px')
  await expect(groups.last()).toHaveCSS('border-bottom-width', '0px')
  await expect(groups.last()).toHaveCSS('border-top-width', '0px')
})

test('switches to the dark look and remembers it', async ({ launch }) => {
  const app = await launch({ colorScheme: 'light', state: { textA: 'old', textB: 'new' } })

  await app.gear.click()
  await app.page.locator('#pref-theme').selectOption('dark')
  await expect(app.page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await app.expectSaved((state) => state.theme === 'dark')

  // The panes are painted by CodeMirror (src/dark-theme.ts) and the status bar
  // by style.css; the palette has to reach both, not just the chrome.
  await expect(app.pane('a')).toHaveCSS('background-color', 'rgb(21, 21, 21)')
  await expect(app.pane('b')).toHaveCSS('background-color', 'rgb(21, 21, 21)')
  await expect(app.page.locator('#statusbar')).toHaveCSS('background-color', 'rgb(17, 17, 17)')
  // The diff colors come from the dark half of the palette too.
  await expect(app.changedLines('a').first()).toHaveCSS('background-color', 'rgba(248, 81, 73, 0.15)')
  await expect(app.changedLines('b').first()).toHaveCSS('background-color', 'rgba(46, 160, 67, 0.15)')

  // The controls the platform would otherwise draw in its own greys: the
  // mode selector stays flat against the bar, and the pin reads as the
  // secondary text beside it rather than in a fixed grey of its own.
  await expect(app.diffModeSelect).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)')
  await expect(app.alwaysOnTop).toHaveCSS('color', 'rgb(170, 170, 170)')
})

test('applies the font size and pulls out-of-range values back in', async ({ launch }) => {
  const app = await launch()
  const fontSize = app.page.locator('#pref-font-size')

  await app.gear.click()
  await fontSize.fill('40')
  await fontSize.blur()
  await expect(app.pane('a')).toHaveCSS('font-size', '40px')
  await expect(app.pane('b')).toHaveCSS('font-size', '40px')
  await app.expectSaved((state) => state.fontSize === 40)

  await fontSize.fill('999')
  await fontSize.blur()
  await expect(fontSize).toHaveValue('100')
  await app.expectSaved((state) => state.fontSize === 100)
})

test('applies the font weight to both panes and remembers it', async ({ launch }) => {
  const app = await launch()

  await app.gear.click()
  await app.page.locator('#pref-font-weight').selectOption('700')
  // The theme puts the weight on the scroller; the text takes it by inheritance.
  await expect(app.editorA).toHaveCSS('font-weight', '700')
  await expect(app.editorB).toHaveCSS('font-weight', '700')
  await app.expectSaved((state) => state.fontWeight === 700)
})

test('pulls an out-of-range tab width back in', async ({ launch }) => {
  const app = await launch()
  const tabSize = app.page.locator('#pref-tab-size')

  await app.gear.click()
  await tabSize.fill('0')
  await tabSize.blur()
  await expect(tabSize).toHaveValue('1')
  await app.expectSaved((state) => state.tabSize === 1)
})

test('offers the fonts the backend reports', async ({ launch }) => {
  const app = await launch({ fonts: ['BIZ UDGothic', 'Consolas'] })

  await app.gear.click()
  await expect(app.page.locator('#font-list option')).toHaveCount(2)
  await expect(app.page.locator('#font-list option').first()).toHaveAttribute('value', 'BIZ UDGothic')
})

test('covers the panes, which stop taking the pointer', async ({ launch }) => {
  const app = await launch({ state: { textA: 'covered' } })

  await app.gear.click()
  await expect(app.preferences).toBeVisible()

  // The panel is a modal dialog, so the top layer puts it over the panes and
  // a click on the text underneath lands on the dim instead.
  const box = (await app.editorA.boundingBox())!
  const point = { x: box.x + 10, y: box.y + 10 }
  const topmost = await app.page.evaluate(({ x, y }) => document.elementFromPoint(x, y)?.id, point)
  expect(topmost).toBe('preferences')

  await app.page.mouse.click(point.x, point.y)
  await expect(app.preferences).toBeHidden()
})

test("dims the window with the palette alone, not the browser's own backdrop", async ({ launch }) => {
  const app = await launch({ colorScheme: 'light' })

  await app.gear.click()
  await expect(app.preferences).toHaveCSS('background-color', 'rgba(0, 0, 0, 0.35)')
  // Browsers paint ::backdrop themselves, and it would sit under that colour
  // and deepen it by a different amount on each engine.
  const backdrop = await app.page.evaluate(
    () => getComputedStyle(document.getElementById('preferences')!, '::backdrop').backgroundColor,
  )
  expect(backdrop).toBe('rgba(0, 0, 0, 0)')
})

test('marks the spaces and tabs in both panes, and nothing else', async ({ launch }) => {
  // One of each on the left, two of each on the right, so the whole chunk is a
  // difference in whitespace alone — the case the marks exist for.
  const app = await launch({ state: { textA: 'a b\tc\u3000d', textB: 'a  b\t\tc\u3000\u3000d' } })
  const showWhitespace = app.page.locator('#pref-show-whitespace')

  await expect(app.whitespaceMarks('a')).toHaveCount(0)

  await app.gear.click()
  await expect(showWhitespace).not.toBeChecked()
  await showWhitespace.check()
  await app.expectSaved((state) => state.showWhitespace)
  await app.page.keyboard.press('Escape')

  for (const [side, each] of [
    ['a', 1],
    ['b', 2],
  ] as const) {
    await expect(app.pane(side).locator('.cm-highlightSpace')).toHaveCount(each)
    await expect(app.pane(side).locator('.cm-highlightTab')).toHaveCount(each)
    await expect(app.pane(side).locator('.cm-ideographicSpace')).toHaveCount(each)
    // The line break and the end of the text carry no mark: those three are all
    // there is on the line, and the pane holds nothing else.
    await expect(app.whitespaceMarks(side)).toHaveCount(each * 3)
  }

  // The tab's arrow is painted by clipping its span to that shape, so anything
  // the merge view drew inside the span would be clipped away with it. The two
  // never nest that way round — the changed-character mark is the outer one —
  // and this is what says so.
  const nested = await app.pane('b').locator('.cm-highlightTab').first().evaluate((span) => span.childElementCount)
  expect(nested).toBe(0)

  await app.gear.click()
  await showWhitespace.uncheck()
  await expect(app.whitespaceMarks('a')).toHaveCount(0)
  await expect(app.whitespaceMarks('b')).toHaveCount(0)
  await app.expectSaved((state) => !state.showWhitespace)
})

test('paints the whitespace marks from the palette, in both themes', async ({ launch }) => {
  const app = await launch({ colorScheme: 'light', state: { showWhitespace: true, textA: 'a b\tc' } })
  const space = app.pane('a').locator('.cm-highlightSpace')
  const tab = app.pane('a').locator('.cm-highlightTab')

  // The dot is a gradient and the arrow a mask over a fill, so the colour shows
  // up in a different property for each; both come from --whitespace.
  await expect(space).toHaveCSS('background-image', /rgba\(31, 35, 40, 0\.26\)/)
  await expect(tab).toHaveCSS('background-color', 'rgba(31, 35, 40, 0.26)')
  await expect(tab).toHaveCSS('mask-image', /url\("data:image\/svg\+xml/)

  await app.gear.click()
  await app.page.locator('#pref-theme').selectOption('dark')
  await expect(space).toHaveCSS('background-image', /rgba\(216, 216, 216, 0\.26\)/)
  await expect(tab).toHaveCSS('background-color', 'rgba(216, 216, 216, 0.26)')
})
