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

test('switches the grammar from the pane bar and remembers it', async ({ launch }) => {
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

/** The title bar's background, height and the rule under it, beside the pane bar's background. */
async function surfaces(app: App) {
  return app.page.evaluate(() => {
    const style = (selector: string) => getComputedStyle(document.querySelector(selector)!)
    const bar = style('#titlebar')
    return {
      color: bar.backgroundColor,
      height: bar.height,
      seam: bar.borderBottomWidth,
      paneBar: style('#pane-head-a .pane-bar').backgroundColor,
    }
  })
}

/** Where each of the title bar's buttons sits, left to right, as its id. */
async function titleBarOrder(app: App): Promise<string[]> {
  return app.page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('#titlebar button')]
      .filter((button) => button.getBoundingClientRect().width > 0)
      .sort((a, b) => a.getBoundingClientRect().left - b.getBoundingClientRect().left)
      .map((button) => button.id),
  )
}

// The spec draws the two as one bar: painted alike, with no rule between.
for (const [platform, height] of [
  ['macos', '28px'],
  ['windows', '32px'],
] as const) {
  test(`joins the ${platform} title bar to the pane bar, in both themes`, async ({ launch }) => {
    const app = await launch({ platform, colorScheme: 'light' })
    expect(await surfaces(app)).toEqual({ color: 'rgb(246, 248, 250)', height, seam: '0px', paneBar: 'rgb(246, 248, 250)' })

    await app.gear.click()
    await app.page.locator('#pref-theme').selectOption('dark')
    expect(await surfaces(app)).toMatchObject({ color: 'rgb(17, 17, 17)', paneBar: 'rgb(17, 17, 17)' })
  })
}

test('puts the pin and the gear at the left of the Windows title bar, and the window buttons at the right', async ({
  launch,
}) => {
  const app = await launch({ platform: 'windows' })

  expect(await titleBarOrder(app)).toEqual([
    'always-on-top',
    'open-preferences',
    'window-minimize',
    'window-maximize',
    'window-close',
  ])
  const bar = (await app.page.locator('#titlebar').boundingBox())!
  const pin = (await app.alwaysOnTop.boundingBox())!
  const close = (await app.page.locator('#window-close').boundingBox())!
  expect(pin.x - bar.x).toBeLessThan(await token(app, '--caption-button-width'))
  expect(close.x + close.width).toBe(bar.x + bar.width)
})

test('puts the gear and then the pin at the right end of the macOS title bar, clear of the traffic lights', async ({
  launch,
}) => {
  const app = await launch({ platform: 'macos' })

  // The window's buttons are the system's there, drawn over the left end.
  expect(await titleBarOrder(app)).toEqual(['open-preferences', 'always-on-top'])
  const bar = (await app.page.locator('#titlebar').boundingBox())!
  const pin = (await app.alwaysOnTop.boundingBox())!
  expect(bar.x + bar.width - (pin.x + pin.width)).toBeLessThan(await token(app, '--icon-button-size'))
})

test('works the window from the buttons of the Windows title bar', async ({ launch }) => {
  const app = await launch({ platform: 'windows' })
  const maximize = app.page.locator('#window-maximize')

  await app.page.locator('#window-minimize').click()
  await expect.poll(() => app.commands()).toContain('plugin:window|minimize')

  await expect(maximize).toHaveAttribute('aria-label', '最大化')
  await maximize.click()
  await expect.poll(() => app.commands()).toContain('plugin:window|toggle_maximize')
  // The real window says it has changed by resizing; the page is not resized
  // here, so the event stands in for it.
  await app.page.evaluate(() => window.dispatchEvent(new Event('resize')))
  await expect(maximize).toHaveAttribute('aria-label', '元に戻す')

  // None of them took the caret from the editor.
  await app.typeInEditor('閉じる前に残す')
  await app.page.locator('#window-close').click()
  await expect.poll(() => app.commands()).toContain('quit_app')
  expect((await app.saved())?.text).toBe('閉じる前に残す')
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

// The fifth button, 選択範囲, asks for a selection on top of a match, so it is
// left out here and has the test below to itself.
test('turns the buttons off while the search has nothing to act on', async ({ launch }) => {
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

test('leaves the selection-only replace off until a range holds a whole match', async ({ launch }) => {
  const app = await launch()
  const inSelection = app.searchButton('replaceSelection')

  await app.typeInEditor('alpha beta\nalpha gamma')
  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()
  await app.typeInSearch('alpha')

  // Two matches to act on, but the caret is a point: there is no range to
  // replace within, so this one button stays off while the other four are on.
  await expect(app.searchCount).toHaveText('2 件')
  await expect(app.searchButton('replaceAll')).toBeEnabled()
  await expect(inSelection).toBeDisabled()

  // A range of the second line that stops short of its match.
  await app.editor.click()
  await app.page.keyboard.press('Control+End')
  for (let i = 0; i < 'gamma'.length; i++) await app.page.keyboard.press('Shift+ArrowLeft')
  await expect(inSelection).toBeDisabled()

  // Grown to the whole line, which holds one of the two matches.
  await app.page.keyboard.press('Shift+Home')
  await expect(inSelection).toBeEnabled()
})

test('replaces within the selected range and leaves the rest of the draft alone', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('alpha\nalpha')
  await app.page.keyboard.press('Control+Home')
  await app.page.keyboard.press('Shift+End')

  // CodeMirror opens the panel on the selected text, which is the query here.
  await app.press('KeyF')
  await expect(app.searchField).toHaveValue('alpha')
  await app.searchPanel.getByPlaceholder('置換').click()
  await app.page.keyboard.type('beta')

  await app.searchButton('replaceSelection').click()
  await app.expectSaved((state) => state.text === 'beta\nalpha')

  // The whole pass is one entry in the history, the way すべて is, and the
  // press lands in the draft because the button handed the keyboard back.
  await app.press('KeyZ')
  await app.expectSaved((state) => state.text === 'alpha\nalpha')
})

// The query CodeMirror hands a multi-line cursor — one holding \n — used to come
// back from a range with nothing at all: that cursor leaves its read position
// where it is when a match is turned down, so a single match before the range
// ended the scan. src/search-panel.ts asks the cursor for the range instead.
test('replaces within the selection when the query spans lines', async ({ launch }) => {
  const app = await launch({ state: { searchRegexp: true, text: '- one\n- two\n\n- three\n- four' } })

  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()
  await app.typeInSearch('\\n- ')
  await expect(app.searchCount).toHaveText('3 件')

  // The last line and the one above it: the only match inside is the one
  // between them. The two in the first list come before the range.
  await app.editor.click()
  await app.page.keyboard.press('Control+End')
  await app.page.keyboard.press('Shift+ArrowUp')
  await app.page.keyboard.press('Shift+Home')

  await app.searchPanel.getByPlaceholder('置換').click()
  await app.page.keyboard.type(' / ')
  await app.searchButton('replaceSelection').click()

  await app.expectSaved((state) => state.text === '- one\n- two\n\n- three / four')
})

test('says how many matches a pass replaced, until the draft moves on', async ({ launch }) => {
  const app = await launch()
  const replaced = app.searchPanel.locator('.cm-replace-count')

  await app.typeInEditor('alpha alpha alpha')
  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()
  await app.typeInSearch('alpha')
  await app.searchPanel.getByPlaceholder('置換').click()
  await app.page.keyboard.type('beta')

  // Nothing has been replaced yet, so the replace field's tail says nothing.
  await expect(replaced).toHaveText('')
  await app.searchButton('replaceAll').click()
  await expect(replaced).toHaveText('3 件置換')

  // The figure stands for one pass over one draft; the next keystroke in the
  // draft leaves it counting nothing, so it goes.
  await app.page.keyboard.type('x')
  await expect(replaced).toHaveText('')
})

test('hands the keyboard back to the draft after a replacement', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('alpha alpha')
  await app.press('KeyF')
  await expect(app.searchPanel).toBeVisible()
  await app.typeInSearch('alpha')
  await app.searchPanel.getByPlaceholder('置換').click()
  await app.page.keyboard.type('beta')
  await expect(app.searchPanel.getByPlaceholder('置換')).toBeFocused()

  // Pressed with the keyboard in the panel; it ends up in the draft, so that
  // the undo that follows takes back the replacement rather than the typing.
  // The first press steps to the match and the second replaces it, which is
  // how CodeMirror's 置換 has always worked.
  await app.searchButton('replace').click()
  await expect(app.editor).toBeFocused()
  await app.searchButton('replace').click()
  await app.expectSaved((state) => state.text === 'beta alpha')
  await app.press('KeyZ')
  await app.expectSaved((state) => state.text === 'alpha alpha')
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
  const muted = await app.page.evaluate(() => getComputedStyle(document.querySelector('.pane-bar')!).color)
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

  // The two fields are one column and the five buttons the other; each row
  // opens at the column's left edge and its buttons touch, so 前へ stands over
  // 置換 and 次へ over 選択範囲. Nothing here is a wrapper CodeMirror gives us,
  // so the edges are what says the grid held.
  expect(await edges(app.searchField)).toEqual(await edges(app.searchPanel.getByPlaceholder('置換')))
  expect(await edges(app.searchButton('prev'))).toEqual(await edges(app.searchButton('replace')))
  expect(await edges(app.searchButton('next'))).toEqual(await edges(app.searchButton('replaceSelection')))
  expect((await edges(app.searchButton('prev'))).right).toBe((await edges(app.searchButton('next'))).left)
  expect((await edges(app.searchButton('replaceSelection'))).right).toBe(
    (await edges(app.searchButton('replaceAll'))).left,
  )
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
