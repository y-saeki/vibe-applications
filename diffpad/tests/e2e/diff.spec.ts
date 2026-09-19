// Editing the two panes and what the diff between them shows.

import { type App, expect, test } from './fixtures'

test('says the panes agree until one of them is edited', async ({ launch }) => {
  const app = await launch()

  await expect(app.diffCount).toHaveText('差異なし')
  await expect(app.previousDiff).toBeDisabled()
  await expect(app.nextDiff).toBeDisabled()

  await app.typeInEditor('a', 'alpha')
  await expect(app.diffCount).toHaveText('差異 1 箇所')
  await expect(app.previousDiff).toBeEnabled()
  await expect(app.nextDiff).toBeEnabled()

  // Bringing the right pane into line takes the chunk away again.
  await app.typeInEditor('b', 'alpha')
  await expect(app.diffCount).toHaveText('差異なし')
  await expect(app.nextDiff).toBeDisabled()
})

test('marks the lines of a chunk on both sides, and the characters only in char mode', async ({ launch }) => {
  const app = await launch({ state: { textA: 'foo bar\nsame', textB: 'foo baz\nsame', diffMode: 'line' } })

  await expect(app.changedLines('a')).toHaveCount(1)
  await expect(app.changedLines('b')).toHaveCount(1)
  await expect(app.changedText('a')).toHaveCount(0)
  await expect(app.changedText('b')).toHaveCount(0)

  await app.diffModeSelect.selectOption('char')
  await expect(app.page.locator('#editor')).toHaveAttribute('data-diff-mode', 'char')
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

test('paints the two sides in their own colors', async ({ launch }) => {
  const app = await launch({ colorScheme: 'light', state: { textA: 'old', textB: 'new' } })

  // Taken out on the left, put in on the right: the usual red and green, with
  // the characters in a stronger wash than the line.
  await expect(app.changedLines('a').first()).toHaveCSS('background-color', 'rgb(255, 235, 233)')
  await expect(app.changedLines('b').first()).toHaveCSS('background-color', 'rgb(218, 251, 225)')
  await expect(app.changedText('a').first()).toHaveCSS('background-color', 'rgba(255, 129, 130, 0.4)')
  await expect(app.changedText('b').first()).toHaveCSS('background-color', 'rgba(74, 194, 107, 0.4)')
  await expect(app.pane('a').locator('.cm-changedLineGutter').first()).toHaveCSS('background-color', 'rgb(207, 34, 46)')
  await expect(app.pane('b').locator('.cm-changedLineGutter').first()).toHaveCSS('background-color', 'rgb(26, 127, 55)')
})

test('hands both texts to the backend without being asked to save', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('a', 'あとで読む')
  await app.typeInEditor('b', 'あとで比べる')
  await app.expectSaved((state) => state.textA === 'あとで読む' && state.textB === 'あとで比べる')
})

test('writes the texts out when the window loses the focus', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('a', '急いで書いた')
  await app.page.evaluate(() => window.dispatchEvent(new Event('blur')))
  await app.expectSaved((state) => state.textA === '急いで書いた')
})

test('keeps the last value when a save fails and the text changes again', async ({ launch }) => {
  const app = await launch({ failSave: 'disk is full' })

  await app.typeInEditor('a', '消えないで')
  // The save was attempted and rejected; the pane still holds the text, and
  // nothing half-written was recorded as the saved state.
  await expect.poll(() => app.commands()).toContain('save_state')
  await expect(app.editorA).toContainText('消えないで')
  expect(await app.saved()).toBeNull()
})

test('undoes and redoes from the keyboard, in the pane that has it', async ({ launch }) => {
  const app = await launch({ state: { textA: 'left' } })

  await app.typeInEditor('b', 'first')
  await app.press('KeyZ')
  await expect(app.editorB).not.toContainText('first')
  await expect(app.editorA).toContainText('left')
  await app.page.keyboard.press(`${app.mod}+Shift+KeyZ`)
  await expect(app.editorB).toContainText('first')
})

test('indents with spaces, as many as the tab width, in either pane', async ({ launch }) => {
  const app = await launch({ state: { tabSize: 2 } })

  await app.typeInEditor('b', 'x')
  await app.page.keyboard.press('Home')
  await app.page.keyboard.press('Tab')
  await app.expectSaved((state) => state.textB === '  x')
})

test('steps through the chunks with the buttons, and round again at the end', async ({ launch }) => {
  const app = await launch({ state: { textA: 'one\ntwo\nthree\nfour', textB: 'one\nTWO\nthree\nFOUR' } })

  await expect(app.diffCount).toHaveText('差異 2 箇所')
  expect(await app.caretLine('a')).toBe(1)

  await app.nextDiff.click()
  expect(await app.caretLine('a')).toBe(2)
  // The keyboard goes back to the pane, so the next thing typed lands there.
  await expect(app.editorA).toBeFocused()
  await app.nextDiff.click()
  expect(await app.caretLine('a')).toBe(4)
  await app.nextDiff.click()
  expect(await app.caretLine('a')).toBe(2)

  await app.previousDiff.click()
  expect(await app.caretLine('a')).toBe(4)
})

test('steps in whichever pane last had the keyboard', async ({ launch }) => {
  const app = await launch({ state: { textA: 'one\ntwo\nthree', textB: 'one\nTWO\nthree' } })

  await app.editorB.click({ position: { x: 4, y: 4 } })
  await expect(app.editorB).toBeFocused()
  await app.nextDiff.click()
  expect(await app.caretLine('b')).toBe(2)
  expect(await app.caretLine('a')).toBe(0)
})

test('takes a click below the last line as a click on that pane', async ({ launch }) => {
  const app = await launch({ state: { textA: 'short', textB: 'short' } })

  // The panes are as tall as the window, not as tall as their text, so the
  // empty space under a short text still belongs to a pane.
  const box = (await app.pane('b').boundingBox())!
  await app.page.mouse.click(box.x + box.width / 2, box.y + box.height - 20)
  await expect(app.editorB).toBeFocused()
  expect(await app.caretLine('b')).toBe(1)
})

test('keeps the list shut until it is asked for', async ({ launch }) => {
  const app = await launch()
  const firstOption = app.diffModeSelect.locator('option').first()

  // The browser hides a closed picker with `display: none`, and a `display`
  // written for it in style.css beats that: the list then sits open over the
  // window from the moment the app loads.
  await expect(firstOption).toBeHidden()

  await app.diffModeSelect.click()
  // A list the platform owns is a window of its own, which never enters the
  // page whether it is open or not.
  if (await app.listIsOurs()) await expect(firstOption).toBeVisible()
})

test('leaves the list to macOS', async ({ launch }) => {
  const app = await launch({ platform: 'macos' })

  // The menu macOS opens already looks like the rest of that system, so the
  // base appearance is asked for on Windows only.
  expect(await app.listIsOurs()).toBe(false)
})

test('remembers the always-on-top toggle and passes it to the window', async ({ launch }) => {
  const app = await launch()

  await app.alwaysOnTop.click()
  await expect(app.alwaysOnTop).toHaveAttribute('aria-pressed', 'true')
  await app.expectSaved((state) => state.alwaysOnTop)
  const call = (await app.calls()).find((entry) => entry.cmd === 'plugin:window|set_always_on_top')
  expect(call?.args).toMatchObject({ value: true })
})

test('keeps the bar still however many digits the count runs to', async ({ launch }) => {
  // Every other line differs: 100 chunks, the widest reading the cell is
  // built for.
  const lines = Array.from({ length: 200 }, (_, i) => `line ${i}`)
  const app = await launch({
    state: { textA: lines.join('\n'), textB: lines.map((line, i) => (i % 2 ? `LINE ${i}` : line)).join('\n') },
  })

  await expect(app.diffCount).toHaveText('差異 100 箇所')
  // The cell is held at the width its token gives it, separator included, so
  // the longest reading does not push it wider than the shortest one.
  expect((await app.diffCount.boundingBox())?.width).toBe(await token(app, '--count-width'))
  const wide = await app.nextDiff.boundingBox()

  // Which is what the bar is really being asked for: emptying a pane takes the
  // count from its widest reading to its shortest, and the buttons do not move.
  await app.editorB.click({ position: { x: 4, y: 4 } })
  await app.press('KeyA')
  await app.page.keyboard.press('Backspace')
  await expect(app.diffCount).toHaveText('差異 1 箇所')
  expect((await app.nextDiff.boundingBox())?.x).toBe(wide?.x)
})

/** A length token as the sheet declares it, in px. */
function token(app: App, name: string): Promise<number> {
  return app.page.evaluate(
    (property) => Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue(property)),
    name,
  )
}
