import { expect, test, windowMinimum } from './fixtures'

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
    '#pref-indent-style',
    '#pref-quick-suggestions',
    '#pref-theme',
    '#pref-font-family',
    '#pref-font-weight',
    '#pref-font-size',
    '#pref-show-whitespace',
    '#pref-show-indent-guides',
    '#pref-show-line-numbers',
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
  expect(await groupOf('#pref-show-indent-guides')).toBe('表示')
  expect(await groupOf('#pref-show-line-numbers')).toBe('表示')

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

  // The editor is painted by CodeMirror (src/dark-theme.ts) and the title bar
  // by style.css; the palette has to reach both, not just the chrome.
  await expect(app.page.locator('.cm-editor')).toHaveCSS('background-color', 'rgb(21, 21, 21)')
  await expect(app.page.locator('#titlebar')).toHaveCSS('background-color', 'rgb(17, 17, 17)')

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

test('switches indenting between spaces and tabs', async ({ launch }) => {
  const app = await launch({ state: { tabSize: 2 } })
  const indentStyle = app.page.locator('#pref-indent-style')

  await app.gear.click()
  await expect(indentStyle).toHaveValue('spaces')
  await indentStyle.selectOption('tabs')
  await app.expectSaved((state) => state.indentStyle === 'tabs')
  await app.page.keyboard.press('Escape')

  await app.typeInEditor('x')
  await app.page.keyboard.press('Home')
  await app.page.keyboard.press('Tab')
  await app.expectSaved((state) => state.text === '\tx')
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

test('rules every line that is inside an indented block, and no other', async ({ launch }) => {
  // A block two levels deep, with a blank line inside it and another line at
  // the margin under it, so that both of the cases a blank line can be in are
  // in the same draft.
  const app = await launch({ state: { text: 'a\n    b\n        c\n\n    d\ne\n' } })
  const showIndentGuides = app.page.locator('#pref-show-indent-guides')

  await expect(app.indentGuides).toHaveCount(0)

  await app.gear.click()
  await expect(showIndentGuides).not.toBeChecked()
  await showIndentGuides.check()
  await app.expectSaved((state) => state.showIndentGuides)
  await app.page.keyboard.press('Escape')

  // A line that starts at the margin is in no block and takes nothing, and the
  // blank line between the two indented ones takes the shallower of them so
  // that the block's own rule runs on through it. The one under the block,
  // with nothing indented below it, takes none.
  expect(await app.indentGuideLevels()).toEqual(['1', '2', '1', '1'])

  await app.gear.click()
  await showIndentGuides.uncheck()
  await expect(app.indentGuides).toHaveCount(0)
  await app.expectSaved((state) => !state.showIndentGuides)
})

test('counts a level as the tab width, and recounts when that changes', async ({ launch }) => {
  const app = await launch({ state: { showIndentGuides: true, text: 'a\n    b\n        c\n\n    d\ne\n' } })
  const stepOf = () =>
    app.editor.evaluate((content) => getComputedStyle(content).getPropertyValue('--indent-guide-step').trim())

  expect(await stepOf()).toBe('4')
  expect(await app.indentGuideLevels()).toEqual(['1', '2', '1', '1'])

  // The same four and eight columns of indentation are twice as many levels at
  // a tab width of 2, and the blank line inside the block follows them.
  await app.gear.click()
  await app.page.locator('#pref-tab-size').fill('2')
  await app.page.keyboard.press('Escape')

  expect(await stepOf()).toBe('2')
  expect(await app.indentGuideLevels()).toEqual(['2', '4', '2', '2'])
})

test('paints the indentation rules from the palette, in both themes', async ({ launch }) => {
  const app = await launch({ colorScheme: 'light', state: { showIndentGuides: true, text: 'a\n    b' } })
  const guides = app.indentGuides

  await expect(guides).toHaveCSS('background-image', /rgba\(31, 35, 40, 0\.14\)/)

  await app.gear.click()
  await app.page.locator('#pref-theme').selectOption('dark')
  await expect(guides).toHaveCSS('background-image', /rgba\(216, 216, 216, 0\.14\)/)
})

test('keeps the panel inside its width at the narrowest the window goes', async ({ launch }) => {
  const app = await launch()
  // Hiding the platform's scrollbars takes the horizontal one with it — that
  // cannot be asked for on one axis alone — and draftpad draws no horizontal
  // bar of its own. Nothing here needs one: the label column is fixed and the
  // field beside it takes what is left, down to the narrowest window the app
  // opens at.
  await app.page.setViewportSize({ width: windowMinimum().width, height: windowMinimum().height })

  await app.gear.click()
  // What is asked for is that nothing is laid out past the panel's content
  // edge, which is the whole of what a horizontal bar would have been for.
  // Not that the panel measures no wider than itself: WebKit puts about 11px
  // of scrollable width over the theme row's native select with no box of any
  // kind in it, which `.panel` clips rather than scrolls (see style.css).
  //
  // Everything measured comes back together, so that a failure names what
  // stuck out instead of leaving the next reader to measure by hand. `own`
  // and the panel's three widths do not decide it; they are there to read.
  const { box, ...overflow } = await app.preferences.locator('.panel').evaluate((element: HTMLElement) => {
    const inside =
      element.getBoundingClientRect().left +
      element.clientLeft +
      element.clientWidth -
      Number.parseFloat(getComputedStyle(element).paddingRight)
    return {
      spilling: [...element.querySelectorAll('*')]
        .map((child) => ({
          what:
            child.tagName.toLowerCase() +
            (child.id ? `#${child.id}` : '') +
            (typeof child.className === 'string' && child.className ? `.${child.className.trim().split(/\s+/).join('.')}` : ''),
          past: Math.round(child.getBoundingClientRect().right - inside),
          own: child.scrollWidth - child.clientWidth,
          wide: Math.round(child.getBoundingClientRect().width),
        }))
        .filter((child) => child.past > 0),
      box: { scroll: element.scrollWidth, client: element.clientWidth, offset: element.offsetWidth },
    }
  })
  expect(overflow, `panel ${JSON.stringify(box)}`).toEqual({ spilling: [] })
})

test('lays a bar over the panel when the window is too short to hold it', async ({ launch }) => {
  const app = await launch()
  // Short enough that the nine settings no longer fit between the panel's
  // insets, whatever else is on screen.
  await app.page.setViewportSize({ width: 600, height: 300 })

  await app.gear.click()
  const panel = app.preferences.locator('.panel')
  const bar = app.scrollbar('preferences')

  const box = await panel.evaluate((element: HTMLElement) => ({
    overflows: element.scrollHeight > element.clientHeight,
    // Its own border, and no column taken out for a scrollbar beside it.
    taken: element.offsetWidth - element.clientWidth,
    right: element.getBoundingClientRect().right,
  }))
  expect(box.overflows).toBe(true)
  expect(box.taken).toBe(2)

  await panel.evaluate((element: HTMLElement) => {
    element.scrollTop = 40
  })
  await expect(bar).toHaveAttribute('data-shown', '')
  // Inside the panel's own edge rather than beyond it: the bar is drawn within
  // the dialog, which is the only thing the top layer lets over it.
  const strip = (await bar.boundingBox())!
  expect(strip.x + strip.width).toBeLessThanOrEqual(box.right)
  expect(strip.x + strip.width).toBeGreaterThan(box.right - strip.width - 2)
})

test('numbers the lines of the draft, and counts them the way the pane bar does', async ({ launch }) => {
  const app = await launch({ state: { text: 'a\nb\nc' } })
  const showLineNumbers = app.page.locator('#pref-show-line-numbers')

  await expect(app.lineNumbers).toHaveCount(0)

  await app.gear.click()
  await expect(showLineNumbers).not.toBeChecked()
  await showLineNumbers.check()
  await app.expectSaved((state) => state.showLineNumbers)
  await app.page.keyboard.press('Escape')

  await expect(app.lineNumberCells).toHaveText(['1', '2', '3'])
  await expect(app.lines).toHaveText('3 行')

  // A line the draft gains is a line the column gains.
  await app.typeInEditor('\nd')
  await expect(app.lineNumberCells).toHaveText(['1', '2', '3', '4'])

  await app.gear.click()
  await showLineNumbers.uncheck()
  await expect(app.lineNumbers).toHaveCount(0)
  await app.expectSaved((state) => !state.showLineNumbers)
})

test('gives a wrapped line one number, at its top', async ({ launch }) => {
  // The draft wraps every line (EditorView.lineWrapping), so the first of these
  // two is drawn on several rows and the second on one.
  const app = await launch({ state: { showLineNumbers: true, text: `${'wrap '.repeat(80)}\nlast` } })

  // Two lines in the draft, so two numbers, however many rows they take.
  await expect(app.lineNumberCells).toHaveText(['1', '2'])

  const cell = (await app.lineNumberCells.first().boundingBox())!
  const wrapped = (await app.page.locator('.cm-line').first().boundingBox())!
  const single = (await app.page.locator('.cm-line').last().boundingBox())!
  expect(wrapped.height).toBeGreaterThan(single.height * 2)

  // The cell covers the whole of the line it counts, and its number sits at the
  // top of it: the column is laid out in the draft's lines, not in the rows
  // they are drawn on.
  expect(Math.abs(cell.height - wrapped.height)).toBeLessThan(2)
  expect(Math.abs(cell.y - wrapped.y)).toBeLessThan(2)
})

test('draws the column as a margin rather than a panel, in both themes', async ({ launch }) => {
  // The whitespace setting is on so that a mark and a number are on screen
  // together: the two are meant to carry the same value.
  const app = await launch({
    colorScheme: 'light',
    state: { showLineNumbers: true, showWhitespace: true, text: 'a b\nc' },
  })
  const gutters = app.page.locator('.cm-gutters')

  // CodeMirror's own theme fills the gutter and rules it off from the draft.
  // Neither survives: what stands beside the draft is the figures.
  await expect(gutters).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)')
  await expect(gutters).toHaveCSS('border-right-width', '0px')
  // The marks' own value, which --line-number points at rather than repeating:
  // a number is something to find when it is looked for, the same as a mark,
  // and the draft is the only thing above either of them.
  await expect(gutters).toHaveCSS('color', 'rgba(31, 35, 40, 0.26)')
  await expect(app.page.locator('.cm-highlightSpace').first()).toHaveCSS('background-image', /rgba\(31, 35, 40, 0\.26\)/)
  await expect(app.editor).toHaveCSS('color', 'rgb(31, 35, 40)')

  // Right-aligned, so the ones column stands where the draft begins, and the
  // narrow gap is the one on that side.
  await expect(app.lineNumberCells.first()).toHaveCSS('text-align', 'right')
  await expect(app.lineNumberCells.first()).toHaveCSS('padding-right', '4px')
  await expect(app.lineNumberCells.first()).toHaveCSS('padding-left', '12px')

  await app.gear.click()
  await app.page.locator('#pref-theme').selectOption('dark')
  await expect(gutters).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)')
  // The dark block redefines --whitespace and nothing else; the column follows
  // it there because that is what its own token resolves to.
  await expect(gutters).toHaveCSS('color', 'rgba(216, 216, 216, 0.26)')
})

test('sizes the numbers with the draft', async ({ launch }) => {
  const app = await launch({ state: { showLineNumbers: true, fontSize: 24, text: 'a' } })

  // The gutter sits inside the scroller, so it takes the editor's font as well:
  // a number stays on the line it counts at any size.
  await expect(app.lineNumbers).toHaveCSS('font-size', '24px')

  await app.gear.click()
  await app.page.locator('#pref-font-size').fill('40')
  await app.page.keyboard.press('Escape')
  await expect(app.lineNumbers).toHaveCSS('font-size', '40px')
})
