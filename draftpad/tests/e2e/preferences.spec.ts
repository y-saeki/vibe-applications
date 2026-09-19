import { expect, test } from './fixtures'

test('opens from the gear button and closes with Escape', async ({ launch }) => {
  const app = await launch()

  await expect(app.preferences).toBeHidden()
  await app.gear.click()
  await expect(app.preferences).toBeVisible()

  await app.page.keyboard.press('Escape')
  await expect(app.preferences).toBeHidden()
  // Closing hands the keyboard back, so typing goes into the draft again.
  await expect(app.editor).toBeFocused()
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
  await expect(version).toHaveText('draftpad 9.8.7')
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
    '#pref-mode',
    '#pref-tab-size',
    '#pref-quick-suggestions',
    '#pref-theme',
    '#pref-font-family',
    '#pref-font-weight',
    '#pref-font-size',
    '#pref-show-whitespace',
  ]
  await app.page.locator('#pref-mode').focus()
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
  expect(await groupOf('#pref-quick-suggestions')).toBe('編集')
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

test('flips the suggestions toggle from the keyboard, and remembers it', async ({ launch }) => {
  const app = await launch({ state: { quickSuggestions: true } })
  const suggestions = app.page.locator('#pref-quick-suggestions')

  await app.gear.click()
  await expect(suggestions).toBeChecked()

  // It is still a checkbox, whatever it is painted as: Tab reaches it and
  // Space is what works it.
  await suggestions.focus()
  await app.page.keyboard.press('Space')
  await expect(suggestions).not.toBeChecked()
  await app.expectSaved((state) => state.quickSuggestions === false)

  await app.page.keyboard.press('Space')
  await expect(suggestions).toBeChecked()
  await app.expectSaved((state) => state.quickSuggestions === true)
})

test('draws that toggle as a switch the width of two controls', async ({ launch }) => {
  const app = await launch({ colorScheme: 'light', state: { quickSuggestions: false } })
  const suggestions = app.page.locator('#pref-quick-suggestions')

  await app.gear.click()
  // The track is the silhouette of the select and the number field beside it,
  // widened: same height, same corner, same border.
  const box = (await suggestions.boundingBox())!
  expect(box.width).toBe(48)
  expect(box.height).toBe(28)
  await expect(suggestions).toHaveCSS('border-radius', '6px')

  // Pressing anywhere on the track works it, not just the knob.
  await suggestions.click({ position: { x: 44, y: 24 } })
  await expect(suggestions).toBeChecked()

  // The knob is what moves and what the accent rides on; the track does not
  // change under it. The knob slides rather than jumping, so these are polled:
  // read straight after the click they catch it part of the way across.
  const knob = (property: string) =>
    expect
      .poll(() =>
        app.page.evaluate(
          (name) =>
            getComputedStyle(document.querySelector('#pref-quick-suggestions')!, '::before').getPropertyValue(name),
          property,
        ),
      )
  await knob('translate').toBe('20px')
  await knob('background-color').toBe('rgb(43, 108, 176)')
  await expect(suggestions).toHaveCSS('background-color', 'rgb(238, 241, 244)')

  await suggestions.uncheck()
  await knob('translate').toBe('none')
  // Off, the knob is a white face the border keeps apart from the track.
  await knob('background-color').toBe('rgb(255, 255, 255)')
  await knob('border-top-color').toBe('rgb(208, 215, 222)')
  await expect(suggestions).toHaveCSS('background-color', 'rgb(238, 241, 244)')

  // Dark has no white face to set the knob apart, so lightness does it instead
  // and the outline goes away rather than darkening an already dark knob.
  await app.page.locator('#pref-theme').selectOption('dark')
  await knob('background-color').toBe('rgb(51, 51, 51)')
  await knob('border-top-color').toBe('rgba(0, 0, 0, 0)')
  await expect(suggestions).toHaveCSS('background-color', 'rgb(28, 28, 28)')
})

test('switches to the dark look and remembers it', async ({ launch }) => {
  const app = await launch({ colorScheme: 'light' })

  await app.gear.click()
  await app.page.locator('#pref-theme').selectOption('dark')
  await expect(app.page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await app.expectSaved((state) => state.theme === 'dark')

  // The editor is painted by CodeMirror (src/dark-theme.ts) and the status bar
  // by style.css; the palette has to reach both, not just the chrome.
  await expect(app.page.locator('.cm-editor')).toHaveCSS('background-color', 'rgb(21, 21, 21)')
  await expect(app.page.locator('#statusbar')).toHaveCSS('background-color', 'rgb(17, 17, 17)')

  // The controls the platform would otherwise draw in its own greys: the
  // language selector stays flat against the bar, and the pin reads as the
  // secondary text beside it rather than in a fixed grey of its own.
  await expect(app.languageSelect).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)')
  await expect(app.alwaysOnTop).toHaveCSS('color', 'rgb(170, 170, 170)')
})

test('applies the font size and pulls out-of-range values back in', async ({ launch }) => {
  const app = await launch()
  const fontSize = app.page.locator('#pref-font-size')

  await app.gear.click()
  await fontSize.fill('40')
  await fontSize.blur()
  await expect(app.page.locator('.cm-editor')).toHaveCSS('font-size', '40px')
  await app.expectSaved((state) => state.fontSize === 40)

  await fontSize.fill('999')
  await fontSize.blur()
  await expect(fontSize).toHaveValue('100')
  await app.expectSaved((state) => state.fontSize === 100)
})

test('applies the font weight and remembers it', async ({ launch }) => {
  const app = await launch()

  await app.gear.click()
  await app.page.locator('#pref-font-weight').selectOption('700')
  // The theme puts the weight on the scroller; the text takes it by inheritance.
  await expect(app.editor).toHaveCSS('font-weight', '700')
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

test('turns Vim mode on, and the editor stops taking plain typing', async ({ launch }) => {
  const app = await launch()

  await app.gear.click()
  await app.page.locator('#pref-mode').selectOption('vim')
  await app.expectSaved((state) => state.editorMode === 'vim')
  await app.page.keyboard.press('Escape')
  await expect(app.editor).toBeFocused()

  // Normal mode: a letter is a command, so nothing reaches the draft.
  await app.page.keyboard.press('j')
  await expect(app.chars).toHaveText('0 文字')

  // "i" switches to insert mode, and it is text again.
  await app.page.keyboard.press('i')
  await app.typeInEditor('書けた')
  await app.expectSaved((state) => state.text === '書けた')
})

test('starts in Vim mode when that is what was saved', async ({ launch }) => {
  const app = await launch({ state: { editorMode: 'vim' } })

  await expect(app.page.locator('.cm-scroller')).toHaveClass(/cm-vimMode/)
})

test('covers the search bar, which closes it like the rest of the backdrop', async ({ launch }) => {
  const app = await launch()

  await app.press('f')
  await expect(app.searchField).toBeVisible()
  await app.gear.click()
  await expect(app.preferences).toBeVisible()

  // CodeMirror stacks its own panels above the page; the panel is a modal
  // dialog, so the top layer puts it over them and the search bar greys out
  // and stops taking the pointer like the rest of the window.
  const box = (await app.searchField.boundingBox())!
  const point = { x: box.x + box.width / 2, y: box.y + box.height / 2 }
  const topmost = await app.page.evaluate(({ x, y }) => document.elementFromPoint(x, y)?.id, point)
  expect(topmost).toBe('preferences')

  await app.page.mouse.click(point.x, point.y)
  await expect(app.preferences).toBeHidden()
  await expect(app.searchField).not.toBeFocused()
})

test('dims the window with the palette alone, not the browser\'s own backdrop', async ({ launch }) => {
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

test('marks the spaces and tabs in the draft, and nothing else', async ({ launch }) => {
  const app = await launch({ state: { text: 'a b\tc\u3000d\ne' } })
  const showWhitespace = app.page.locator('#pref-show-whitespace')

  await expect(app.whitespaceMarks).toHaveCount(0)

  await app.gear.click()
  await expect(showWhitespace).not.toBeChecked()
  await showWhitespace.check()
  await app.expectSaved((state) => state.showWhitespace)
  await app.page.keyboard.press('Escape')

  await expect(app.page.locator('.cm-highlightSpace')).toHaveCount(1)
  await expect(app.page.locator('.cm-highlightTab')).toHaveCount(1)
  await expect(app.page.locator('.cm-ideographicSpace')).toHaveCount(1)
  // The line break and the end of the draft carry no mark: those three are all
  // there is on the first line, and the second holds nothing to mark.
  await expect(app.whitespaceMarks).toHaveCount(3)

  // The tab's arrow is painted by clipping its span to that shape, so anything
  // drawn inside the span would be clipped away with it. Nothing ever is: a
  // search match or a selection match wraps the whitespace mark rather than
  // sitting inside it, and this is what says so.
  const nested = await app.page.locator('.cm-highlightTab').evaluate((span) => span.childElementCount)
  expect(nested).toBe(0)

  await app.gear.click()
  await showWhitespace.uncheck()
  await expect(app.whitespaceMarks).toHaveCount(0)
  await app.expectSaved((state) => !state.showWhitespace)
})

test('paints the whitespace marks from the palette, in both themes', async ({ launch }) => {
  const app = await launch({ colorScheme: 'light', state: { showWhitespace: true, text: 'a b\tc' } })
  const space = app.page.locator('.cm-highlightSpace')
  const tab = app.page.locator('.cm-highlightTab')

  // The dot is a gradient and the arrow a mask over a fill, so the colour shows
  // up in a different property for each; both come from --whitespace.
  await expect(space).toHaveCSS('background-image', /rgba\(31, 35, 40, 0\.26\)/)
  // The two stops sit apart, which is what gives the dot a soft edge. A hard
  // one rasterises differently in each space of a run, because their boxes land
  // on different halves of a pixel; style.css says more. This pins the
  // declaration, not the pixels — those are for the manual pass.
  await expect(space).toHaveCSS('background-image', /0\.26\) 8%, rgba\(0, 0, 0, 0\) 20%/)
  await expect(tab).toHaveCSS('background-color', 'rgba(31, 35, 40, 0.26)')
  await expect(tab).toHaveCSS('mask-image', /url\("data:image\/svg\+xml/)

  await app.gear.click()
  await app.page.locator('#pref-theme').selectOption('dark')
  await expect(space).toHaveCSS('background-image', /rgba\(216, 216, 216, 0\.26\)/)
  await expect(tab).toHaveCSS('background-color', 'rgba(216, 216, 216, 0.26)')
})
