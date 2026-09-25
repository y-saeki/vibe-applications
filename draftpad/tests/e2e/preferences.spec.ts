import { expect, test, windowMinimum } from './fixtures'

test('opens from the title bar button and closes with Escape', async ({ launch }) => {
  const app = await launch()

  await expect(app.preferences).toBeHidden()
  await app.openPreferences.click()
  await expect(app.preferences).toBeVisible()

  await app.page.keyboard.press('Escape')
  await expect(app.preferences).toBeHidden()
  // Closing hands the keyboard back, so typing goes into the draft again.
  await expect(app.editor).toBeFocused()
})

test('closes when the backdrop is clicked', async ({ launch }) => {
  const app = await launch()

  await app.openPreferences.click()
  await expect(app.preferences).toBeVisible()
  await app.preferences.click({ position: { x: 4, y: 4 } })
  await expect(app.preferences).toBeHidden()
})

test('shows the version, set apart from the settings above it', async ({ launch }) => {
  const app = await launch({ version: '9.8.7' })

  await app.openPreferences.click()
  const version = app.page.locator('#pref-version')
  await expect(version).toHaveText('draftpad 9.8.7')
  // Not a setting, so it is deliberately outside the label column: bottom
  // right, in figures whose width does not move as the number changes.
  await expect(version).toHaveCSS('text-align', 'right')
  await expect(version).toHaveCSS('font-family', /ui-monospace|SFMono-Regular|Menlo|Consolas|monospace/)
})

test('gathers the settings into the two groups, in that order', async ({ launch }) => {
  const app = await launch()

  await app.openPreferences.click()
  await expect(app.page.locator('#pref-general legend')).toHaveText(['編集', '表示'])

  // The order in the panel is the order the keyboard walks them in, so one
  // check covers both.
  const ids = [
    '#pref-indent-style',
    '#pref-tab-size',
    '#pref-theme',
    '#pref-font-family',
    '#pref-font-weight',
    '#pref-font-size',
    '#pref-show-whitespace',
    '#pref-show-indent-guides',
    '#pref-show-line-numbers',
  ]
  await app.page.locator('#pref-indent-style').focus()
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
  expect(await groupOf('#pref-show-indent-guides')).toBe('表示')
  expect(await groupOf('#pref-show-line-numbers')).toBe('表示')

  // One rule between the two, and it hangs off the group above: a fieldset's
  // block-start border is the one its legend notches and sits on, so a
  // border-top would be drawn straight through the heading.
  const groups = app.page.locator('#pref-general .pref-group')
  await expect(groups).toHaveCount(2)
  await expect(groups.first()).toHaveCSS('border-bottom-width', '1px')
  await expect(groups.first()).toHaveCSS('border-top-width', '0px')
  await expect(groups.last()).toHaveCSS('border-bottom-width', '0px')
  await expect(groups.last()).toHaveCSS('border-top-width', '0px')
})

test('flips a toggle from the keyboard, and remembers it', async ({ launch }) => {
  const app = await launch({ state: { showWhitespace: true } })
  const toggle = app.page.locator('#pref-show-whitespace')

  await app.openPreferences.click()
  await expect(toggle).toBeChecked()

  // It is still a checkbox, whatever it is painted as: Tab reaches it and
  // Space is what works it.
  await toggle.focus()
  await app.page.keyboard.press('Space')
  await expect(toggle).not.toBeChecked()
  await app.expectSaved((state) => state.showWhitespace === false)

  await app.page.keyboard.press('Space')
  await expect(toggle).toBeChecked()
  await app.expectSaved((state) => state.showWhitespace === true)
})

test('draws a toggle as a switch the width of two controls', async ({ launch }) => {
  const app = await launch({ colorScheme: 'light', state: { showWhitespace: false } })
  const toggle = app.page.locator('#pref-show-whitespace')

  await app.openPreferences.click()
  // The track is the silhouette of the select and the number field beside it,
  // widened: same height, same corner, same border.
  const box = (await toggle.boundingBox())!
  expect(box.width).toBe(48)
  expect(box.height).toBe(28)
  await expect(toggle).toHaveCSS('border-radius', '6px')

  // Pressing anywhere on the track works it, not just the knob.
  await toggle.click({ position: { x: 44, y: 24 } })
  await expect(toggle).toBeChecked()

  // The knob is what moves and what the accent rides on; the track does not
  // change under it. The knob slides rather than jumping, so these are polled:
  // read straight after the click they catch it part of the way across.
  const knob = (property: string) =>
    expect
      .poll(() =>
        app.page.evaluate(
          (name) =>
            getComputedStyle(document.querySelector('#pref-show-whitespace')!, '::before').getPropertyValue(name),
          property,
        ),
      )
  await knob('translate').toBe('20px')
  await knob('background-color').toBe('rgb(43, 108, 176)')
  await expect(toggle).toHaveCSS('background-color', 'rgb(238, 241, 244)')

  await toggle.uncheck()
  await knob('translate').toBe('none')
  // Off, the knob is a white face the border keeps apart from the track.
  await knob('background-color').toBe('rgb(255, 255, 255)')
  await knob('border-top-color').toBe('rgb(208, 215, 222)')
  await expect(toggle).toHaveCSS('background-color', 'rgb(238, 241, 244)')

  // Dark has no white face to set the knob apart, so lightness does it instead
  // and the outline goes away rather than darkening an already dark knob.
  await app.page.locator('#pref-theme').selectOption('dark')
  await knob('background-color').toBe('rgb(51, 51, 51)')
  await knob('border-top-color').toBe('rgba(0, 0, 0, 0)')
  await expect(toggle).toHaveCSS('background-color', 'rgb(28, 28, 28)')
})

test('switches to the dark look and remembers it', async ({ launch }) => {
  const app = await launch({ colorScheme: 'light' })

  await app.openPreferences.click()
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

  await app.openPreferences.click()
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

  await app.openPreferences.click()
  await app.page.locator('#pref-font-weight').selectOption('700')
  // The theme puts the weight on the scroller; the text takes it by inheritance.
  await expect(app.editor).toHaveCSS('font-weight', '700')
  await app.expectSaved((state) => state.fontWeight === 700)
})

test('pulls an out-of-range tab width back in', async ({ launch }) => {
  const app = await launch()
  const tabSize = app.page.locator('#pref-tab-size')

  await app.openPreferences.click()
  await tabSize.fill('0')
  await tabSize.blur()
  await expect(tabSize).toHaveValue('1')
  await app.expectSaved((state) => state.tabSize === 1)
})

test('switches indenting between spaces and tabs', async ({ launch }) => {
  const app = await launch({ state: { tabSize: 2 } })
  const indentStyle = app.page.locator('#pref-indent-style')

  await app.openPreferences.click()
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

  await app.openPreferences.click()
  await expect(app.page.locator('#font-list option')).toHaveCount(2)
  await expect(app.page.locator('#font-list option').first()).toHaveAttribute('value', 'BIZ UDGothic')
})

test('covers the search bar, which closes it like the rest of the backdrop', async ({ launch }) => {
  const app = await launch()

  await app.press('f')
  await expect(app.searchField).toBeVisible()
  await app.openPreferences.click()
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

  await app.openPreferences.click()
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

  await app.openPreferences.click()
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

  await app.openPreferences.click()
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

  await app.openPreferences.click()
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

  await app.openPreferences.click()
  await expect(showIndentGuides).not.toBeChecked()
  await showIndentGuides.check()
  await app.expectSaved((state) => state.showIndentGuides)
  await app.page.keyboard.press('Escape')

  // A line that starts at the margin is in no block and takes nothing, and the
  // blank line between the two indented ones takes the shallower of them so
  // that the block's own rule runs on through it. The one under the block,
  // with nothing indented below it, takes none.
  expect(await app.indentGuideLevels()).toEqual(['1', '2', '1', '1'])

  await app.openPreferences.click()
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
  await app.openPreferences.click()
  await app.page.locator('#pref-tab-size').fill('2')
  await app.page.keyboard.press('Escape')

  expect(await stepOf()).toBe('2')
  expect(await app.indentGuideLevels()).toEqual(['2', '4', '2', '2'])
})

test('paints the indentation rules from the palette, in both themes', async ({ launch }) => {
  const app = await launch({ colorScheme: 'light', state: { showIndentGuides: true, text: 'a\n    b' } })
  const guides = app.indentGuides

  await expect(guides).toHaveCSS('background-image', /rgba\(31, 35, 40, 0\.14\)/)

  await app.openPreferences.click()
  await app.page.locator('#pref-theme').selectOption('dark')
  await expect(guides).toHaveCSS('background-image', /rgba\(216, 216, 216, 0\.14\)/)
})

test('keeps the panel inside its width at the narrowest the window goes', async ({ launch }) => {
  // Two of the longest keys Windows writes out on one shortcut, so that the
  // widest row the tab can hold is drawn.
  const app = await launch({ state: { shortcuts: { preferences: ['Alt+Shift+Mod+,', 'Alt+Shift+Mod+F12'] } } })
  // Hiding the platform's scrollbars takes the horizontal one with it — that
  // cannot be asked for on one axis alone — and draftpad draws no horizontal
  // bar of its own. Nothing here needs one: the label column is fixed and the
  // field beside it takes what is left, down to the narrowest window the app
  // opens at; on the shortcut tab, the names wrap and so do the keys.
  await app.page.setViewportSize({ width: windowMinimum().width, height: windowMinimum().height })

  await app.openPreferences.click()
  // What is asked for is that nothing is laid out past the panel's content
  // edge, which is the whole of what a horizontal bar would have been for.
  // Not that the panel measures no wider than itself: WebKit puts about 11px
  // of scrollable width over the theme row's native select with no box of any
  // kind in it, which `.panel-body` clips rather than scrolls (see style.css).
  //
  // The content edge is the panel's, less the inset every part keeps from
  // it. The body and the shortcut tab's foot are the two boxes that run to
  // the panel's edge themselves — the one so that its bar is drawn there, the
  // other for its rule — so they are measured by what is inside them.
  //
  // Everything measured comes back together, so that a failure names what
  // stuck out instead of leaving the next reader to measure by hand. `own`
  // and the panel's three widths do not decide it; they are there to read.
  const measure = () =>
    app.preferences.locator('.panel').evaluate((element: HTMLElement) => {
      const inset = Number.parseFloat(getComputedStyle(element.querySelector('.panel-body')!).paddingRight)
      const inside = element.getBoundingClientRect().left + element.clientLeft + element.clientWidth - inset
      return {
        spilling: [...element.querySelectorAll('*')]
          .filter((child) => !child.matches('.panel-body, .shortcut-foot') && child.getClientRects().length > 0)
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

  const { box, ...overflow } = await measure()
  expect(overflow, `panel ${JSON.stringify(box)}`).toEqual({ spilling: [] })

  await app.preferences.locator('#pref-tab-shortcuts').click()
  await expect(app.shortcutRow('preferences').locator('.key-button')).toHaveCount(2)
  const { box: shortcutBox, ...shortcutOverflow } = await measure()
  expect(shortcutOverflow, `panel ${JSON.stringify(shortcutBox)}`).toEqual({ spilling: [] })
})

test('lays a bar over the panel when the window is too short to hold it', async ({ launch }) => {
  const app = await launch()
  // Short enough that the nine settings no longer fit between the panel's
  // insets, whatever else is on screen.
  await app.page.setViewportSize({ width: 600, height: 300 })

  await app.openPreferences.click()
  // The heading and the tabs stay; what scrolls is the body under them, which
  // runs to the panel's edge.
  const body = app.preferences.locator('.panel-body')
  const bar = app.scrollbar('preferences')

  const box = await body.evaluate((element: HTMLElement) => ({
    overflows: element.scrollHeight > element.clientHeight,
    // No column taken out for a scrollbar beside it.
    taken: element.offsetWidth - element.clientWidth,
    right: element.closest('.panel')!.getBoundingClientRect().right,
  }))
  expect(box.overflows).toBe(true)
  expect(box.taken).toBe(0)
  await expect(app.preferences.locator('.pref-tabs')).toBeInViewport()

  await body.evaluate((element: HTMLElement) => {
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

  await app.openPreferences.click()
  await expect(showLineNumbers).not.toBeChecked()
  await showLineNumbers.check()
  await app.expectSaved((state) => state.showLineNumbers)
  await app.page.keyboard.press('Escape')

  await expect(app.lineNumberCells).toHaveText(['1', '2', '3'])
  await expect(app.lines).toHaveText('3 行')

  // A line the draft gains is a line the column gains.
  await app.typeInEditor('\nd')
  await expect(app.lineNumberCells).toHaveText(['1', '2', '3', '4'])

  await app.openPreferences.click()
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

  await app.openPreferences.click()
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

  await app.openPreferences.click()
  await app.page.locator('#pref-font-size').fill('40')
  await app.page.keyboard.press('Escape')
  await expect(app.lineNumbers).toHaveCSS('font-size', '40px')
})

// ---- the shortcut tab ------------------------------------------------------

test('lists every shortcut, the ones that cannot be changed under headings marked 変更不可', async ({ launch }) => {
  const app = await launch()

  await app.openShortcuts()
  await expect(app.preferences.locator('#pref-shortcuts legend')).toHaveText([
    'アプリ',
    '編集',
    '選択・移動',
    '検索',
    'その他',
    '基本の編集変更不可',
    'カーソル移動変更不可',
    'ウィンドウ変更不可',
  ])
  await expect(app.shortcutRow('find').locator('.key-button')).toHaveText(['Ctrl+F'])
  await expect(app.shortcutRow('find_next').locator('.key-button')).toHaveText(['Ctrl+G', 'F3'])
  // A fixed key is read, not worked: no control on its row.
  const copy = app.preferences.locator('.shortcut-row.fixed', { hasText: 'コピー' })
  await expect(copy.locator('kbd')).toHaveText(['Ctrl+C'])
  await expect(copy.locator('button')).toHaveCount(0)
  // Nothing has been moved, so there is nothing to count and nothing to put back.
  await expect(app.preferences.locator('#shortcut-count')).toHaveText('')
  await expect(app.preferences.locator('#shortcut-reset-all')).toBeDisabled()
})

test('writes the keys the macOS way on macOS, and lists what only macOS has', async ({ launch }) => {
  const app = await launch({ platform: 'macos' })

  await app.openShortcuts()
  await expect(app.shortcutRow('paste_plain').locator('.key-button')).toHaveText(['⇧⌘V'])
  // Full screen is an item AppKit draws, which keeps its own key.
  await expect(app.shortcutRow('toggle_fullscreen')).toHaveCount(0)
  await expect(app.preferences.locator('.shortcut-row.fixed', { hasText: 'フルスクリーンを切り替え' }).locator('kbd')).toHaveText(['⌃⌘F'])
  await expect(app.preferences.locator('#pref-shortcuts legend', { hasText: 'macOS のテキスト操作' })).toBeVisible()
})

test('switches tabs with the arrow keys, and opens again on the tab it was left on', async ({ launch }) => {
  const app = await launch()

  await app.openPreferences.click()
  await app.preferences.locator('#pref-tab-general').focus()
  await app.page.keyboard.press('ArrowRight')
  await expect(app.preferences.locator('#pref-tab-shortcuts')).toHaveAttribute('aria-selected', 'true')
  await expect(app.preferences.locator('#pref-general')).toBeHidden()
  await expect(app.preferences.locator('#shortcut-filter')).toBeVisible()

  await app.page.keyboard.press('Escape')
  await expect(app.preferences).toBeHidden()
  await app.openPreferences.click()
  await expect(app.preferences.locator('#pref-shortcuts')).toBeVisible()
  await expect(app.preferences.locator('#shortcut-filter')).toBeFocused()
})

test('moves a shortcut to the key pressed after clicking it', async ({ launch }) => {
  const app = await launch()
  const find = app.shortcutRow('find')

  await app.openShortcuts()
  await find.locator('.key-button').click()
  await expect(find.locator('.key-button')).toHaveText('キーを入力…')
  await app.page.keyboard.press('Control+Shift+KeyF')
  await expect(find.locator('.key-button')).toHaveText('Ctrl+Shift+F')
  await expect(find.locator('.modified-mark')).toHaveText('変更済み')
  await expect(app.preferences.locator('#shortcut-count')).toHaveText('1 件を変更済み')
  // Only what was changed is kept, so a key draftpad moves later still reaches
  // every shortcut the user left alone.
  await app.expectSaved((state) => JSON.stringify(state.shortcuts) === JSON.stringify({ find: ['Shift+Mod+F'] }))
})

test('takes a key the window would act on while it waits, and Escape without closing the panel', async ({ launch }) => {
  const app = await launch()
  const find = app.shortcutRow('find')

  await app.openShortcuts()
  await find.locator('.key-button').click()
  // Ctrl+W closes the window everywhere else.
  await app.page.keyboard.press('Control+KeyW')
  await expect(find.locator('.shortcut-message')).toBeVisible()
  expect(await app.commands()).not.toContain('quit_app')
  await find.getByRole('button', { name: 'キャンセル' }).click()

  await find.locator('.key-button').click()
  await app.page.keyboard.press('Escape')
  await expect(find.locator('.key-button')).toHaveText('Ctrl+F')
  await expect(app.preferences).toBeVisible()
})

test('refuses a key that is typing, and one that cannot be changed', async ({ launch }) => {
  const app = await launch()
  const find = app.shortcutRow('find')

  await app.openShortcuts()
  await find.locator('.key-button').click()
  await app.page.keyboard.press('Shift+KeyK')
  await expect(find.locator('.shortcut-message')).toHaveText('Ctrl か Alt を含む組み合わせか、ファンクションキーを押してください。')

  await find.locator('.key-button').click()
  await app.page.keyboard.press('Control+KeyC')
  await expect(find.locator('.shortcut-message')).toHaveText('Ctrl+C は「コピー」に使われているため、割り当てられません。')
  await expect(find.locator('.key-button')).toHaveText('Ctrl+F')
})

test('asks before taking a key from another shortcut, and leaves it alone on キャンセル', async ({ launch }) => {
  const app = await launch()
  const find = app.shortcutRow('find')
  const deleteLine = app.shortcutRow('delete_line')

  await app.openShortcuts()
  await find.locator('.key-button').click()
  await app.page.keyboard.press('Control+Shift+KeyK')
  await expect(find.locator('.shortcut-message')).toContainText(
    'Ctrl+Shift+K は「行を削除」に割り当て済みです。置き換えると、「行を削除」は未設定になります。',
  )
  await find.getByRole('button', { name: 'キャンセル' }).click()
  await expect(find.locator('.shortcut-message')).toHaveCount(0)
  await expect(deleteLine.locator('.key-button')).toHaveText('Ctrl+Shift+K')

  await find.locator('.key-button').click()
  await app.page.keyboard.press('Control+Shift+KeyK')
  await find.getByRole('button', { name: '置き換える' }).click()
  await expect(find.locator('.key-button')).toHaveText('Ctrl+Shift+K')
  await expect(deleteLine.locator('.key-button')).toHaveText('未設定')
  await app.expectSaved(
    (state) => JSON.stringify(state.shortcuts) === JSON.stringify({ delete_line: [], find: ['Shift+Mod+K'] }),
  )
})

test('says which keys the other shortcut keeps when it has more than one', async ({ launch }) => {
  const app = await launch()
  const find = app.shortcutRow('find')

  await app.openShortcuts()
  await find.locator('.key-button').click()
  await app.page.keyboard.press('F3')
  await expect(find.locator('.shortcut-message')).toContainText(
    'F3 は「次を検索」に割り当て済みです。置き換えると、「次を検索」からは F3 が外れます。',
  )
})

test('takes a key off with ×, and puts the defaults back with ↺', async ({ launch }) => {
  const app = await launch()
  const find = app.shortcutRow('find')

  await app.openShortcuts()
  await expect(find.locator('.reset-button')).toHaveCount(0)
  await find.getByRole('button', { name: 'Ctrl+F を外す' }).click()
  await expect(find.locator('.key-button')).toHaveText('未設定')
  await app.expectSaved((state) => JSON.stringify(state.shortcuts) === JSON.stringify({ find: [] }))

  // The empty slot takes a key the same way a key does.
  await find.locator('.key-button').click()
  await app.page.keyboard.press('Control+KeyJ')
  await expect(find.locator('.key-button')).toHaveText('Ctrl+J')

  await find.getByRole('button', { name: '検索・置換 を初期設定に戻す' }).click()
  await expect(find.locator('.key-button')).toHaveText('Ctrl+F')
  await expect(find.locator('.reset-button')).toHaveCount(0)
  await app.expectSaved((state) => JSON.stringify(state.shortcuts) === '{}')
})

// Resetting one shortcut must not hand out a key that another has since been
// given: that is how two shortcuts would end up on one key.
test('asks before ↺ takes a default back from the shortcut that has it now', async ({ launch }) => {
  const app = await launch({ state: { shortcuts: { find: [], delete_line: ['Mod+F'] } } })
  const find = app.shortcutRow('find')

  await app.openShortcuts()
  await find.getByRole('button', { name: '検索・置換 を初期設定に戻す' }).click()
  await expect(find.locator('.shortcut-message')).toContainText(
    'Ctrl+F は「行を削除」に割り当て済みです。初期設定に戻すと、「行を削除」は未設定になります。',
  )
  await find.getByRole('button', { name: '初期設定に戻す', exact: true }).click()
  await expect(find.locator('.key-button')).toHaveText('Ctrl+F')
  await expect(app.shortcutRow('delete_line').locator('.key-button')).toHaveText('未設定')
  await app.expectSaved((state) => JSON.stringify(state.shortcuts) === JSON.stringify({ delete_line: [] }))
})

test('puts every shortcut back on the second press of すべて初期設定に戻す', async ({ launch }) => {
  const app = await launch({ state: { shortcuts: { find: ['Shift+Mod+F'], quit: [] } } })
  const resetAll = app.preferences.locator('#shortcut-reset-all')

  await app.openShortcuts()
  await expect(app.preferences.locator('#shortcut-count')).toHaveText('2 件を変更済み')
  await resetAll.click()
  await expect(resetAll).toHaveText('もう一度押すと戻します')
  await expect(app.shortcutRow('find').locator('.key-button')).toHaveText('Ctrl+Shift+F')

  await resetAll.click()
  await expect(app.shortcutRow('find').locator('.key-button')).toHaveText('Ctrl+F')
  await expect(app.shortcutRow('quit').locator('.key-button')).toHaveText('Ctrl+Q')
  await expect(app.preferences.locator('#shortcut-count')).toHaveText('')
  await expect(resetAll).toBeDisabled()
  await expect(resetAll).toHaveText('すべて初期設定に戻す')
  await app.expectSaved((state) => JSON.stringify(state.shortcuts) === '{}')
})

test('filters the list by name and by key', async ({ launch }) => {
  const app = await launch()
  const filter = app.preferences.locator('#shortcut-filter')

  await app.openShortcuts()
  await filter.fill('インデント')
  await expect(app.preferences.locator('.shortcut-row .shortcut-name')).toHaveText([
    'インデントを増やす',
    'インデントを減らす',
    'インデントを整える',
    'インデント・インデント解除',
  ])

  await filter.fill('ctrl+shift+v')
  await expect(app.preferences.locator('.shortcut-row .shortcut-name')).toHaveText(['プレーンテキストとして貼り付け'])

  await filter.fill('存在しない')
  await expect(app.preferences.locator('.shortcut-empty')).toHaveText('一致するショートカットはありません。')
})
