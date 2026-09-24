// The compare pane: opening it beside the draft, what the two panes show of
// the difference between them, what the settings and the search panel do
// with two panes, and closing either one.

import { type App, expect, type Side, test } from './fixtures'

/** Two panes as they were closed: "b" is only on the left and "e" only on the right. */
const TWO_PANES = { compare: true, text: 'a\nb\nc\nd', compareText: 'a\nc\nd\ne' } as const

test('opens beside the draft as a copy of it, in the window it already has', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('比べる前の下書き')
  await expect(app.paneHeadB).toBeHidden()
  await expect(app.closeA).toBeHidden()
  await app.compareButton.click()

  await expect(app.page.locator('html')).toHaveAttribute('data-layout', 'compare')
  await expect(app.paneHeadB).toBeVisible()
  await expect(app.editorA).toHaveText('比べる前の下書き')
  await expect(app.editorB).toHaveText('比べる前の下書き')
  // The keyboard goes to the new pane: the text to compare against is what is
  // about to be put there.
  await expect(app.editorB).toBeFocused()
  // The button that opened the pane becomes the one that closes this side.
  await expect(app.compareButton).toBeHidden()
  await expect(app.closeA).toBeVisible()
  await app.expectSaved((state) => state.compare && state.text === '比べる前の下書き' && state.compareText === '比べる前の下書き')
  // Nothing to tell apart yet.
  await expect(app.diffAdded).toHaveText('+0')
  await expect(app.diffRemoved).toHaveText('−0')
  // The pane is made room for inside the window; its size is not touched.
  expect(await app.commands()).not.toContain('plugin:window|set_size')
})

test('gives each pane exactly half of the window', async ({ launch }) => {
  const app = await launch()
  await app.page.setViewportSize({ width: 1200, height: 400 })
  const width = async (selector: string) => (await app.page.locator(selector).boundingBox())!.width

  await app.compareButton.click()
  await expect(app.paneHeadB).toBeVisible()

  // Half each, bars and panes alike, with no pixel taken out of either for the
  // rule between them, and the text as wide in one pane as in the other.
  expect(await width('#pane-head-a')).toBe(600)
  expect(await width('#pane-head-b')).toBe(600)
  expect(await width('.cm-merge-a')).toBe(600)
  expect(await width('.cm-merge-b')).toBe(600)
  expect(await width('.cm-merge-a .cm-content')).toBe(await width('.cm-merge-b .cm-content'))
})

test('pairs the lines up first, and counts what each side has that the other has not', async ({ launch }) => {
  const app = await launch({ state: TWO_PANES })

  // One line taken out on the left, one put in on the right; the lines both
  // have stay level, with a hatched gap opposite each of the two that do not.
  await expect(app.diffRemoved).toHaveText('−1')
  await expect(app.diffAdded).toHaveText('+1')
  await expect(app.changedLines('a')).toHaveText(['b'])
  await expect(app.changedLines('b')).toHaveText(['e'])
  await expect(app.gaps('a')).toHaveCount(1)
  await expect(app.gaps('b')).toHaveCount(1)
  await expect(app.gaps('a').first()).toHaveCSS('background-image', /repeating-linear-gradient/)
  expect(await lineTop(app, 'a', 'c')).toBe(await lineTop(app, 'b', 'c'))
  expect(await lineTop(app, 'a', 'd')).toBe(await lineTop(app, 'b', 'd'))

  // The figures follow the typing: a line added at the end joins the chunk
  // the last one is already in.
  await app.editorB.click()
  await app.page.keyboard.press('Control+End')
  await app.typeInPane('b', '\nx')
  await expect(app.diffAdded).toHaveText('+2')
  await expect(app.diffRemoved).toHaveText('−1')
})

test('counts a line changed on both sides as one taken out and one put in', async ({ launch }) => {
  const app = await launch({ state: { compare: true, text: 'one\ntwo\nthree', compareText: 'one\nTWO\nthree\nfour' } })

  await expect(app.diffRemoved).toHaveText('−1')
  await expect(app.diffAdded).toHaveText('+2')
})

test('marks the lines of a chunk on both sides, and the characters only in char mode', async ({ launch }) => {
  const app = await launch({ state: { compare: true, text: 'foo bar\nsame', compareText: 'foo baz\nsame', diffMode: 'line' } })

  await expect(app.diffModeSelect).toHaveValue('line')
  await expect(app.changedLines('a')).toHaveCount(1)
  await expect(app.changedLines('b')).toHaveCount(1)
  await expect(app.changedText('a')).toHaveCount(0)
  await expect(app.changedText('b')).toHaveCount(0)

  await app.diffModeSelect.selectOption('char')
  // The merge view widens a change to the word around it, so the mark covers
  // "bar" and "baz" rather than the one letter that differs.
  await expect(app.changedText('a')).toHaveText(['bar'])
  await expect(app.changedText('b')).toHaveText(['baz'])
  // The lines stay marked: the characters come on top of them.
  await expect(app.changedLines('a')).toHaveCount(1)
  await app.expectSaved((state) => state.diffMode === 'char')

  await app.diffModeSelect.selectOption('line')
  await expect(app.changedText('a')).toHaveCount(0)
  await app.expectSaved((state) => state.diffMode === 'line')
})

test('paints the two sides in their own colors, with a stripe at the edge of a line', async ({ launch }) => {
  const app = await launch({ colorScheme: 'light', state: { compare: true, text: 'old', compareText: 'new' } })

  // Taken out on the left, put in on the right: the usual red and green, with
  // the characters in a stronger wash than the line.
  await expect(app.changedLines('a').first()).toHaveCSS('background-color', 'rgb(255, 235, 233)')
  await expect(app.changedLines('b').first()).toHaveCSS('background-color', 'rgb(218, 251, 225)')
  await expect(app.changedText('a').first()).toHaveCSS('background-color', 'rgba(255, 129, 130, 0.4)')
  await expect(app.changedText('b').first()).toHaveCSS('background-color', 'rgba(74, 194, 107, 0.4)')
  // The stripe is drawn inside the line rather than in a column beside it, so
  // the text stands where it did with one pane.
  await expect(app.changedLines('a').first()).toHaveCSS('box-shadow', /inset/)
  await expect(app.changedLines('a').first()).toHaveCSS('box-shadow', /rgb\(207, 34, 46\)/)
  await expect(app.changedLines('b').first()).toHaveCSS('box-shadow', /rgb\(26, 127, 55\)/)
  await expect(app.page.locator('.cm-gutters')).toHaveCount(0)
  // The figures in the bar take the same two colors, a step quieter.
  await expect(app.diffAdded).toHaveCSS('color', 'rgb(26, 127, 55)')
  await expect(app.diffRemoved).toHaveCSS('color', 'rgb(207, 34, 46)')
  await expect(app.page.locator('#status-diff')).toHaveCSS('opacity', '0.8')
})

test('rules the two panes apart, in both themes', async ({ launch }) => {
  const app = await launch({ colorScheme: 'light', state: TWO_PANES })
  // The rule is the last thing drawn in the row of bars and in the box the
  // panes are in, so that it lies over the panes' own surfaces: the dark look
  // gives the editor a background, which a shadow on the right pane went
  // under. A pseudo-element is not in the DOM, so its computed style is what
  // there is to read.
  const rule = (selector: string) =>
    app.page.evaluate((target) => {
      const style = getComputedStyle(document.querySelector(target)!, '::after')
      return { content: style.content, position: style.position, width: style.width, left: style.left, color: style.backgroundColor }
    }, selector)

  for (const selector of ['#pane-heads', '#panes']) {
    const box = (await app.page.locator(selector).boundingBox())!
    expect(await rule(selector)).toEqual({
      content: '""',
      position: 'absolute',
      width: '1px',
      left: `${box.width / 2}px`,
      color: 'rgb(208, 215, 222)',
    })
  }

  await app.openPreferences.click()
  await app.page.locator('#pref-theme').selectOption('dark')
  expect((await rule('#panes')).color).toBe('rgb(44, 44, 44)')
  expect((await rule('#pane-heads')).color).toBe('rgb(44, 44, 44)')

  // With one pane there is nothing to rule apart.
  await app.page.keyboard.press('Escape')
  await app.closeB.click()
  await expect(app.page.locator('html')).toHaveAttribute('data-layout', 'single')
  expect((await rule('#panes')).content).toBe('none')
})

test('spaces the end of the bar as the spec draws it', async ({ launch }) => {
  const app = await launch({ state: TWO_PANES })
  const box = async (locator: ReturnType<App['paneBar']>) => (await locator.boundingBox())!

  // The button's glyph stands --space-3 clear of the hairline and of the
  // bar's edge alike: the button is --icon-button-size around an --icon-size
  // glyph, so its own gutter is half of that on either side.
  const gutter = (await token(app, '--icon-button-size') - (await token(app, '--icon-size'))) / 2
  const clear = await token(app, '--space-3')
  for (const [side, button] of [
    ['a', app.closeA],
    ['b', app.closeB],
  ] as const) {
    const bar = await box(app.paneBar(side))
    const lines = await box(side === 'a' ? app.lines : app.linesB)
    const glyph = await box(button)
    expect(glyph.x - (lines.x + lines.width)).toBe(clear - gutter)
    expect(bar.x + bar.width - (glyph.x + glyph.width)).toBe(clear - gutter)
  }

  // The +N / −N figures keep the same distance from the hairline as the line
  // count keeps on its other side, and the words sit close to the next box.
  const stat = await box(app.page.locator('#status-diff'))
  const chars = await box(app.charsB)
  expect(stat.x + stat.width).toBe(chars.x)
  await expect(app.page.locator('#status-diff')).toHaveCSS('padding-right', `${clear}px`)
  await expect(app.linesB).toHaveCSS('padding-right', `${clear}px`)
  await expect(app.linesB).toHaveCSS('padding-left', `${await token(app, '--space-2')}px`)
})

test('keeps a search panel at the foot of its own pane, in view, without a blank under the other', async ({ launch }) => {
  const app = await launch({ state: { compare: true, text: 'short', compareText: 'short' } })
  const box = async (selector: string) => (await app.page.locator(selector).boundingBox())!

  await app.editorA.click()
  await app.press('KeyF')
  await expect(app.searchPanelOf('a')).toBeVisible()

  // The panel closes the left pane off at the bottom of the box, and the right
  // pane runs all the way down beside it: the panel's height is the left
  // pane's alone, not a strip taken out of both.
  const panes = await box('#panes')
  const panel = await box('.cm-merge-a .cm-search')
  const scrollerA = await box('.cm-merge-a .cm-scroller')
  const scrollerB = await box('.cm-merge-b .cm-scroller')
  expect(Math.abs(panel.y + panel.height - (panes.y + panes.height))).toBeLessThanOrEqual(1)
  expect(Math.abs(scrollerA.y + scrollerA.height - panel.y)).toBeLessThanOrEqual(1)
  expect(Math.abs(scrollerB.y + scrollerB.height - (panes.y + panes.height))).toBeLessThanOrEqual(1)
})

test('keeps the search panel in view as the panes scroll', async ({ launch }) => {
  const lines = Array.from({ length: 400 }, (_, line) => `${line + 1} 行目`).join('\n')
  const app = await launch({ state: { compare: true, text: lines, compareText: lines } })
  const box = async (selector: string) => (await app.page.locator(selector).boundingBox())!

  await app.editorB.click()
  await app.press('KeyF')
  await expect(app.searchPanelOf('b')).toBeVisible()
  const panes = await box('#panes')
  const before = await box('.cm-merge-b .cm-search')
  expect(Math.abs(before.y + before.height - (panes.y + panes.height))).toBeLessThanOrEqual(1)

  // The panel is inside the pane, which is as tall as its text; it holds to
  // the bottom of the merge view as that scrolls rather than going with the
  // text.
  await app.page.locator('.cm-mergeView').evaluate((element) => {
    element.scrollTop = 300
  })
  await expect.poll(async () => (await box('.cm-merge-b .cm-search')).y).toBe(before.y)
  await expect(app.searchField).toBeFocused()
})

test('counts each pane on its own', async ({ launch }) => {
  const app = await launch({ state: { compare: true, text: 'あい\nう', compareText: 'えおか' } })

  // The line break counts, as it does with one pane.
  await expect(app.chars).toHaveText('4 文字')
  await expect(app.lines).toHaveText('2 行')
  await expect(app.charsB).toHaveText('3 文字')
  await expect(app.linesB).toHaveText('1 行')

  await app.editorB.click()
  await app.page.keyboard.press('Control+End')
  await app.typeInPane('b', '\nき')
  await expect(app.charsB).toHaveText('5 文字')
  await expect(app.linesB).toHaveText('2 行')
  await expect(app.chars).toHaveText('4 文字')
})

test('closes the right pane and goes on with the left', async ({ launch }) => {
  const app = await launch({ state: TWO_PANES })

  await app.closeB.click()

  await expect(app.page.locator('html')).toHaveAttribute('data-layout', 'single')
  await expect(app.paneHeadB).toBeHidden()
  await expect(app.compareButton).toBeVisible()
  await expect(app.editor.locator('.cm-line')).toHaveText(['a', 'b', 'c', 'd'])
  await expect(app.editor).toBeFocused()
  // The pane that stays takes the whole window, which keeps its size.
  expect(await app.commands()).not.toContain('plugin:window|set_size')
  // What the closed pane held is gone; there is no asking first.
  await app.expectSaved((state) => !state.compare && state.text === 'a\nb\nc\nd' && state.compareText === '')
})

test('closes the left pane and goes on with the right, as the draft', async ({ launch }) => {
  const app = await launch({ state: TWO_PANES })

  await app.closeA.click()

  await expect(app.page.locator('html')).toHaveAttribute('data-layout', 'single')
  await expect(app.editor.locator('.cm-line')).toHaveText(['a', 'c', 'd', 'e'])
  await expect(app.editor).toBeFocused()
  await app.expectSaved((state) => !state.compare && state.text === 'a\nc\nd\ne' && state.compareText === '')
})

test('carries the draft\'s history into the left pane, and starts the right one with none', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('開く前')
  await app.compareButton.click()
  await expect(app.editorB).toBeFocused()
  // The copy has no history of its own to undo.
  await app.press('KeyZ')
  await expect(app.editorB).toHaveText('開く前')

  await app.editorA.click()
  await app.press('KeyZ')
  await expect(app.editorA).toHaveText('')
  await expect(app.editorB).toHaveText('開く前')
  await expect(app.page.locator('html')).toHaveAttribute('data-layout', 'compare')
})

test('undo puts a closed pane back, with the history it had', async ({ launch }) => {
  const app = await launch({ state: { compare: true, text: 'left', compareText: 'right' } })

  await app.typeInPane('b', ' more')
  await app.press('KeyW')
  await expect(app.page.locator('html')).toHaveAttribute('data-layout', 'single')
  await app.expectSaved((state) => !state.compare && state.compareText === '')

  await app.press('KeyZ')
  await expect(app.page.locator('html')).toHaveAttribute('data-layout', 'compare')
  await expect(app.editorA).toHaveText('left')
  await expect(app.editorB).toHaveText('right more')
  await expect(app.editorB).toBeFocused()
  await expect(app.charsB).toHaveText('10 文字')
  await app.expectSaved((state) => state.compare && state.text === 'left' && state.compareText === 'right more')

  // The pane's own history came back with it.
  await app.press('KeyZ')
  await expect(app.editorB).toHaveText('right')
  await expect(app.editorA).toHaveText('left')
})

test('undoes what was typed after the close before putting the pane back', async ({ launch }) => {
  const app = await launch({ state: TWO_PANES })

  await app.closeB.click()
  await app.page.keyboard.press('Control+End')
  await app.typeInEditor('!')
  await expect(app.editor.locator('.cm-line')).toHaveText(['a', 'b', 'c', 'd!'])

  await app.press('KeyZ')
  await expect(app.page.locator('html')).toHaveAttribute('data-layout', 'single')
  await expect(app.editor.locator('.cm-line')).toHaveText(['a', 'b', 'c', 'd'])

  await app.press('KeyZ')
  await expect(app.page.locator('html')).toHaveAttribute('data-layout', 'compare')
  await expect(app.changedLines('b')).toHaveText(['e'])
})

test('puts a closed left pane back on the left', async ({ launch }) => {
  const app = await launch({ state: { compare: true, text: 'left', compareText: 'the right' } })

  await app.closeA.click()
  await expect(app.chars).toHaveText('9 文字')
  await app.press('KeyZ')

  await expect(app.page.locator('html')).toHaveAttribute('data-layout', 'compare')
  await expect(app.editorA).toHaveText('left')
  await expect(app.editorB).toHaveText('the right')
  await expect(app.editorA).toBeFocused()
  // The text that stayed has moved back to the right, and each bar counts its own pane.
  await expect(app.chars).toHaveText('4 文字')
  await expect(app.charsB).toHaveText('9 文字')
  await app.expectSaved((state) => state.compare && state.text === 'left' && state.compareText === 'the right')
})

test('puts back only the pane closed last', async ({ launch }) => {
  const app = await launch({ state: { compare: true, text: 'left', compareText: 'right' } })

  await app.closeB.click()
  await app.compareButton.click()
  await app.closeB.click()
  await app.press('KeyZ')
  // The copy of the draft the second opening made, not the pane before it.
  await expect(app.editorB).toHaveText('left')
  await expect(app.editorB).toBeFocused()
  await app.press('KeyZ')
  await expect(app.editorB).toHaveText('left')
  await expect(app.page.locator('html')).toHaveAttribute('data-layout', 'compare')
})

test('opens the search panel in the pane with the caret, and in that one only', async ({ launch }) => {
  const app = await launch({ state: TWO_PANES })

  await app.editorB.click()
  await expect(app.editorB).toBeFocused()
  await app.press('KeyF')
  await expect(app.searchPanelOf('b')).toBeVisible()
  await expect(app.searchPanelOf('a')).toHaveCount(0)
  await expect(app.searchField).toBeFocused()
  await app.page.keyboard.press('Escape')
  await expect(app.searchPanelOf('b')).toHaveCount(0)

  await app.editorA.click()
  await expect(app.editorA).toBeFocused()
  await app.press('KeyF')
  await expect(app.searchPanelOf('a')).toBeVisible()
  await expect(app.searchPanelOf('b')).toHaveCount(0)
})

test('shares the search toggles between the panes', async ({ launch }) => {
  const app = await launch({ state: TWO_PANES })
  const caseIn = (side: Side) => app.searchPanelOf(side).getByLabel('大文字小文字を区別')
  const regexpIn = (side: Side) => app.searchPanelOf(side).getByLabel('正規表現')

  await app.editorA.click()
  await app.press('KeyF')
  await caseIn('a').check()
  await app.expectSaved((state) => state.searchCaseSensitive && !state.searchRegexp)

  // The other pane's panel opens with the same toggles, and follows a flip
  // made in this one while both are open.
  await app.editorB.click()
  await app.press('KeyF')
  await expect(caseIn('b')).toBeChecked()
  await expect(regexpIn('b')).not.toBeChecked()
  await regexpIn('b').check()
  await expect(regexpIn('a')).toBeChecked()
  await caseIn('b').uncheck()
  await expect(caseIn('a')).not.toBeChecked()
  await app.expectSaved((state) => !state.searchCaseSensitive && state.searchRegexp)
})

test('applies the language and the font to both panes', async ({ launch }) => {
  const app = await launch({ state: TWO_PANES })

  await app.languageSelect.selectOption('rust')
  await expect(app.editorA).toHaveAttribute('data-language', 'rust')
  await expect(app.editorB).toHaveAttribute('data-language', 'rust')

  await app.openPreferences.click()
  await app.page.locator('#pref-font-size').fill('24')
  await app.page.keyboard.press('Escape')
  await expect(app.pane('a')).toHaveCSS('font-size', '24px')
  await expect(app.pane('b')).toHaveCSS('font-size', '24px')
})

test('undoes in the pane that has the keyboard', async ({ launch }) => {
  const app = await launch({ state: { compare: true, text: 'left', compareText: 'right' } })

  await app.typeInPane('b', ' more')
  await expect(app.editorB).toHaveText('right more')
  await app.press('KeyZ')
  await expect(app.editorB).toHaveText('right')
  await expect(app.editorA).toHaveText('left')
})

test('takes a click below the last line as a click on that pane', async ({ launch }) => {
  const app = await launch({ state: { compare: true, text: 'short', compareText: 'short' } })

  // The panes are as tall as the window, not as tall as their text, so the
  // empty space under a short text still belongs to a pane.
  const box = (await app.pane('b').boundingBox())!
  await app.page.mouse.click(box.x + box.width / 2, box.y + box.height - 20)
  await expect(app.editorB).toBeFocused()
})

test('scrolls the two panes together, under one bar', async ({ launch }) => {
  const lines = Array.from({ length: 400 }, (_, line) => `${line + 1} 行目`).join('\n')
  const app = await launch({ state: { compare: true, text: lines, compareText: lines } })
  const bar = app.scrollbar('editor')

  // The merge view is the one thing that scrolls; the bar stands for it.
  const box = await app.page.locator('.cm-mergeView').evaluate((element) => ({
    overflows: element.scrollHeight > element.clientHeight,
    client: element.clientWidth,
    offset: (element as HTMLElement).offsetWidth,
  }))
  expect(box.overflows).toBe(true)
  expect(box.client).toBe(box.offset)
  await expect(bar).not.toHaveAttribute('data-shown')
  const before = await lineTop(app, 'a', '20 行目')

  await app.page.locator('.cm-mergeView').evaluate((element) => {
    element.scrollTop = 300
  })
  await expect(bar).toHaveAttribute('data-shown', '')
  // Both panes moved, by the same amount; the edges are rounded for painting
  // on their own, so a pixel of difference is allowed.
  const after = await lineTop(app, 'a', '20 行目')
  expect(Math.abs(before - 300 - after)).toBeLessThanOrEqual(1)
  expect(await lineTop(app, 'b', '20 行目')).toBe(after)
})

/** Where the line reading `text` starts on the page, in px from the top. */
async function lineTop(app: App, side: Side, text: string): Promise<number> {
  const box = await app.pane(side).locator('.cm-line', { hasText: text }).first().boundingBox()
  return box!.y
}

/** A length token as the sheet declares it, in px. */
function token(app: App, name: string): Promise<number> {
  return app.page.evaluate(
    (property) => Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue(property)),
    name,
  )
}
