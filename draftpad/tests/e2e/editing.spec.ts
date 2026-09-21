import type { Locator } from '@playwright/test'

import { type App, expect, test } from './fixtures'

test('counts what is typed', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('hello')
  await expect(app.chars).toHaveText('5 文字')
  await app.typeInEditor('\nworld')
  await expect(app.chars).toHaveText('11 文字')
  await expect(app.lines).toHaveText('2 行')
})

test('hands the text to the backend without being asked to save', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('あとで読む')
  await app.expectSaved((state) => state.text === 'あとで読む')
})

test('writes the text out when the window loses the focus', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('急いで書いた')
  await app.page.evaluate(() => window.dispatchEvent(new Event('blur')))
  await app.expectSaved((state) => state.text === '急いで書いた')
})

test('keeps the last value when a save fails and the text changes again', async ({ launch }) => {
  const app = await launch({ failSave: 'disk is full' })

  await app.typeInEditor('消えないで')
  // The save was attempted and rejected; the editor still holds the text, and
  // nothing half-written was recorded as the saved state.
  await expect.poll(() => app.commands()).toContain('save_state')
  await expect(app.editor).toContainText('消えないで')
  expect(await app.saved()).toBeNull()
})

test('undoes and redoes from the keyboard', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('first')
  await app.press('KeyZ')
  await expect(app.editor).not.toContainText('first')
  await app.page.keyboard.press(`${app.mod}+Shift+KeyZ`)
  await expect(app.editor).toContainText('first')
})

test('indents with spaces, as many as the tab width', async ({ launch }) => {
  const app = await launch({ state: { tabSize: 2 } })

  await app.typeInEditor('x')
  await app.page.keyboard.press('Home')
  await app.page.keyboard.press('Tab')
  await app.expectSaved((state) => state.text === '  x')
})

test('switches the grammar from the status bar and remembers it', async ({ launch }) => {
  const app = await launch()

  await expect(app.editor).toHaveAttribute('data-language', 'markdown')
  await app.languageSelect.selectOption('rust')
  await expect(app.editor).toHaveAttribute('data-language', 'rust')
  await app.expectSaved((state) => state.language === 'rust')
})

test('keeps the list shut until it is asked for', async ({ launch }) => {
  const app = await launch()
  const firstOption = app.languageSelect.locator('option').first()

  // The browser hides a closed picker with `display: none`, and a `display`
  // written for it in style.css beats that: the list then sits open over the
  // window from the moment the app loads, on every select at once. Nothing
  // else here would notice, because every other case opens the list first.
  await expect(firstOption).toBeHidden()

  await app.languageSelect.click()
  // A list the platform owns is a window of its own, which never enters the
  // page whether it is open or not.
  if (await app.listIsOurs()) await expect(firstOption).toBeVisible()
})

test('leaves the list to macOS', async ({ launch }) => {
  const app = await launch({ platform: 'macos' })

  // The menu macOS opens already looks like the rest of that system, so the
  // base appearance is asked for on Windows only. Without saying so, an engine
  // new enough would take the list over there on its own.
  expect(await app.listIsOurs()).toBe(false)
})

test('picks a language from the dropped-open list', async ({ launch }) => {
  const app = await launch()
  const styleable = await app.page.evaluate(() => CSS.supports('appearance', 'base-select'))
  // Without the base appearance the list is the platform's own window, which
  // nothing inside the page can reach. The case above covers the same choice
  // arriving through the DOM, which is the path that works on both.
  test.skip(!styleable, 'this engine opens the platform list, not one the page can drive')

  await app.languageSelect.click()
  const rust = app.languageSelect.locator('option', { hasText: 'Rust' })
  await expect(rust).toBeVisible()
  await rust.click()

  await expect(app.editor).toHaveAttribute('data-language', 'rust')
  await app.expectSaved((state) => state.language === 'rust')
})

test('walks the dropped-open list with the keyboard', async ({ launch }) => {
  const app = await launch()
  test.skip(!(await app.listIsOurs()), 'the platform owns the list here, so the page cannot drive it')

  // The list is laid out by this sheet once the engine takes the base
  // appearance, so its own styling is what could stop the rows taking the
  // focus. The keyboard is the path that shows it.
  await app.languageSelect.click()
  await expect(app.languageSelect.locator('option').first()).toBeVisible()
  // Two rows down from Markdown: plain text is the one in between, and it
  // carries no grammar, so nothing would be left to assert on the editor.
  await app.page.keyboard.press('ArrowDown')
  await app.page.keyboard.press('ArrowDown')
  await app.page.keyboard.press('Enter')

  await expect(app.editor).toHaveAttribute('data-language', 'yaml')
  await app.expectSaved((state) => state.language === 'yaml')
})

test('remembers the search toggles once they are switched', async ({ launch }) => {
  const app = await launch()

  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()
  await app.searchMatchCase.check()

  await app.expectSaved((state) => state.searchCaseSensitive && !state.searchRegexp)
})

test('remembers the always-on-top toggle and passes it to the window', async ({ launch }) => {
  const app = await launch()

  await app.alwaysOnTop.click()
  await expect(app.alwaysOnTop).toHaveAttribute('aria-pressed', 'true')
  await app.expectSaved((state) => state.alwaysOnTop)
  const call = (await app.calls()).find((entry) => entry.cmd === 'plugin:window|set_always_on_top')
  expect(call?.args).toMatchObject({ value: true })
})

test('counts the matches and numbers the one the search is standing on', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('alpha beta alpha gamma alpha')
  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()
  await app.typeInSearch('alpha')

  // Nothing has been stepped to yet, so there is a total but no number to give
  // the current match.
  await expect(app.searchCount).toHaveText('3 件')
  await app.page.keyboard.press('Enter')
  await expect(app.searchCount).toHaveText('1 / 3 件')
  await app.page.keyboard.press('Enter')
  await expect(app.searchCount).toHaveText('2 / 3 件')
})

test('turns the four buttons off while the search has nothing to act on', async ({ launch }) => {
  const app = await launch()
  const buttons = ['prev', 'next', 'replace', 'replaceAll'] as const

  await app.typeInEditor('alpha beta')
  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()

  // An empty field says nothing at all rather than reporting a miss.
  await expect(app.searchCount).toHaveText('')
  for (const name of buttons) await expect(app.searchButton(name)).toBeDisabled()

  await app.typeInSearch('alpha')
  await expect(app.searchCount).toHaveText('1 件')
  for (const name of buttons) await expect(app.searchButton(name)).toBeEnabled()

  await app.page.keyboard.type('zzz')
  await expect(app.searchCount).toHaveText('一致なし')
  for (const name of buttons) await expect(app.searchButton(name)).toBeDisabled()
})

test('reads a half-written regular expression as a miss, not as an error', async ({ launch }) => {
  const app = await launch({ state: { searchRegexp: true } })

  await app.typeInEditor('alpha (beta)')
  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()
  await app.typeInSearch('(')

  // Half of "(beta)" is a perfectly ordinary thing to have typed so far, so the
  // count says what it found and keeps the panel's own colors.
  await expect(app.searchCount).toHaveText('一致なし')
  const muted = await app.page.evaluate(() => getComputedStyle(document.querySelector('#statusbar')!).color)
  await expect(app.searchCount).toHaveCSS('color', muted)
})

test('lines the search panel up on two columns', async ({ launch }) => {
  const app = await launch({ innerSize: { width: 880, height: 400 } })
  await app.page.setViewportSize({ width: 880, height: 400 })

  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()
  const edges = async (locator: Locator) => {
    const box = (await locator.boundingBox())!
    return { left: Math.round(box.x), right: Math.round(box.x + box.width) }
  }

  // The two fields are one column and the four buttons the other; within a row
  // the pair of buttons touches. Nothing here is a wrapper CodeMirror gives us,
  // so the edges are what says the grid held.
  expect(await edges(app.searchField)).toEqual(await edges(app.searchPanel.getByPlaceholder('置換')))
  expect(await edges(app.searchButton('prev'))).toEqual(await edges(app.searchButton('replace')))
  expect(await edges(app.searchButton('next'))).toEqual(await edges(app.searchButton('replaceAll')))
  expect((await edges(app.searchButton('prev'))).right).toBe((await edges(app.searchButton('next'))).left)
})
test('works the always-on-top toggle from the keyboard, and says which way it is', async ({ launch }) => {
  const app = await launch({ state: { alwaysOnTop: true } })

  // It is a button rather than a checkbox, so the state a screen reader reads
  // out is aria-pressed and nothing else says it.
  await expect(app.alwaysOnTop).toHaveAttribute('aria-pressed', 'true')
  await app.alwaysOnTop.focus()
  await app.page.keyboard.press('Space')
  await expect(app.alwaysOnTop).toHaveAttribute('aria-pressed', 'false')
  await app.expectSaved((state) => !state.alwaysOnTop)

  await app.page.keyboard.press('Enter')
  await expect(app.alwaysOnTop).toHaveAttribute('aria-pressed', 'true')
})

test('keeps the bar still however many digits the counts run to', async ({ launch }) => {
  // 111,999 characters over 1,000 lines: six digits and four, the widest
  // readings the two cells are built for.
  const app = await launch({ state: { text: Array.from({ length: 1000 }, () => 'あ'.repeat(111)).join('\n') } })

  await expect(app.chars).toHaveText('111999 文字')
  await expect(app.lines).toHaveText('1000 行')
  // Each figure is set in a box the width its token gives it, so the longest
  // reading does not push the word after it, or anything beyond, along.
  expect((await app.chars.locator('.count-figure').boundingBox())?.width).toBe(await token(app, '--count-width'))
  expect((await app.lines.locator('.count-figure').boundingBox())?.width).toBe(await token(app, '--count-width-narrow'))

  // Which is what the bar is really being asked for: emptying the draft takes
  // the counts from their widest reading to their shortest, and the button to
  // the right of them does not move.
  const wide = await app.compareButton.boundingBox()
  await app.press('KeyA')
  await app.page.keyboard.press('Backspace')
  await expect(app.chars).toHaveText('0 文字')
  expect((await app.compareButton.boundingBox())?.x).toBe(wide?.x)
})

// A draft long enough that only a fraction of it is ever in view.
const LONG_DRAFT = Array.from({ length: 400 }, (_, line) => `${line + 1} 行目`).join('\n')

/** The scroller's own measurements, which is where a classic bar would show. */
function scroller(app: App): Promise<{
  overflows: boolean
  client: number
  offset: number
  content: number
  top: number
  range: number
}> {
  return app.page.evaluate(() => {
    const box = document.querySelector('.cm-scroller') as HTMLElement
    return {
      overflows: box.scrollHeight > box.clientHeight,
      client: box.clientWidth,
      offset: box.offsetWidth,
      content: box.scrollWidth,
      top: box.scrollTop,
      range: box.scrollHeight - box.clientHeight,
    }
  })
}

/** Scrolls the draft with no pointer involved, the way a key press would. */
function scrollDraft(app: App, top: number): Promise<void> {
  return app.page.evaluate((to) => {
    ;(document.querySelector('.cm-scroller') as HTMLElement).scrollTop = to
  }, top)
}

test('leaves the draft its full width when it outgrows the window', async ({ launch }) => {
  const app = await launch({ state: { text: LONG_DRAFT } })

  // A platform scrollbar would be taking its width out of the scrollport here,
  // which is what makes the wrapping shift as a window is resized. The
  // difference between the two is the border, and .cm-scroller has none.
  const box = await scroller(app)
  expect(box.overflows).toBe(true)
  expect(box.client).toBe(box.offset)
})

test('wraps whatever is in the draft, so it never runs off to the side', async ({ launch }) => {
  // Hiding the platform's scrollbars cannot be asked for on one axis alone, so
  // there is no horizontal bar of draftpad's own to put in their place either.
  // What keeps that from stranding anything is this: with line wrapping on,
  // nothing reaches past the right edge to begin with. The four lines are the
  // shapes that would, if any could — a run with no space in it, the same in
  // full-width characters, a URL, and tabs at the widest tab width the
  // preferences panel offers.
  const app = await launch({
    state: {
      tabSize: 10,
      text: ['a'.repeat(2000), 'あ'.repeat(2000), `https://example.com/${'segment/'.repeat(300)}`, `${'\t'.repeat(200)}x`].join('\n'),
    },
  })

  const box = await scroller(app)
  expect(box.content).toBe(box.client)
})

test('brings the bar up while the draft scrolls, and takes it down after', async ({ launch }) => {
  const app = await launch({ state: { text: LONG_DRAFT } })
  const bar = app.scrollbar('editor')

  await expect(bar).not.toHaveAttribute('data-shown')
  await scrollDraft(app, 200)
  await expect(bar).toHaveAttribute('data-shown', '')
  // And fades out again once the scrolling stops. Where the pointer happens to
  // be rests does not hold it up: it sits over the draft the whole time
  // someone is writing.
  await expect(bar).not.toHaveAttribute('data-shown')
})

test('carries no bar while the whole draft is in view', async ({ launch }) => {
  const app = await launch({ state: { text: '一行だけ' } })

  await expect(app.scrollbar('editor')).toBeHidden()
})

test('gives the thumb the share of the draft in view, and moves it to the end', async ({ launch }) => {
  const app = await launch({ state: { text: LONG_DRAFT } })
  const bar = app.scrollbar('editor')
  const thumb = bar.locator('.scrollbar-thumb')

  // Scrolling is the one thing that brings the bar up, so the far end of the
  // draft is measured first; the draft opens at the top already.
  const { range } = await scroller(app)
  await scrollDraft(app, range)
  await expect(bar).toHaveAttribute('data-shown', '')

  const strip = (await bar.boundingBox())!
  const atEnd = (await thumb.boundingBox())!
  // Only part of the draft is in view, so the thumb covers part of the strip —
  // and never less than the minimum, whatever the draft grows to. The far end
  // of one puts it at the far end of the other; the two edges are rounded for
  // painting on their own, so they may land a pixel apart.
  expect(atEnd.height).toBeLessThan(strip.height)
  expect(atEnd.height).toBeGreaterThanOrEqual(await token(app, '--scrollbar-thumb-min'))
  expect(Math.abs(atEnd.y + atEnd.height - (strip.y + strip.height))).toBeLessThanOrEqual(1)

  await scrollDraft(app, 0)
  await expect
    .poll(async () => Math.abs((await thumb.boundingBox())!.y - strip.y))
    .toBeLessThanOrEqual(1)
})

test('scrolls the draft when the thumb is dragged', async ({ launch }) => {
  const app = await launch({ state: { text: LONG_DRAFT } })
  const bar = app.scrollbar('editor')
  const thumb = bar.locator('.scrollbar-thumb')

  // A scroll puts the bar up; the pointer arriving on the thumb is what keeps
  // it there while it is taken hold of.
  await scrollDraft(app, 200)
  await expect(bar).toHaveAttribute('data-shown', '')

  const grip = (await thumb.boundingBox())!
  await app.page.mouse.move(grip.x + grip.width / 2, grip.y + grip.height / 2)
  await app.page.mouse.down()
  await app.page.mouse.move(grip.x + grip.width / 2, grip.y + grip.height / 2 + 120)
  await app.page.mouse.up()

  expect((await scroller(app)).top).toBeGreaterThan(200)
})

/** A length token as the sheet declares it, in px. */
function token(app: App, name: string): Promise<number> {
  return app.page.evaluate(
    (property) => Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue(property)),
    name,
  )
}
