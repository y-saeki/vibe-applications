// A right click opens draftpad's own menu. The two platforms get there
// differently: macOS cancels the webview's menu and asks the Rust side for a
// native one (src-tauri/src/menu.rs), while Windows keeps the webview's and
// trims it in place (src-tauri/src/context_menu.rs), which the page takes no
// part in.
//
// Either menu is native, so what can be checked here is the half that lives in
// the page: which platform cancels the click, and what the macOS path tells the
// Rust side to enable.

import { type App, expect, test } from './fixtures'

const MAC = { platform: 'macos' } as const

/** Resolves with whether the page had already cancelled the next right click. */
function preventedNextContextMenu(app: App): Promise<boolean> {
  // The page listens in the capture phase, so this one — on the way back up —
  // sees the event as the webview does before deciding to open its menu.
  return app.page.evaluate(
    () =>
      new Promise<boolean>((resolve) => {
        document.addEventListener('contextmenu', (event) => resolve(event.defaultPrevented), { once: true })
      }),
  )
}

test("macOS cancels the webview's menu and asks for draftpad's", async ({ launch }) => {
  const app = await launch(MAC)

  const prevented = preventedNextContextMenu(app)
  await app.editor.click({ button: 'right' })

  expect(await prevented).toBe(true)
  await expect.poll(() => app.commands()).toContain('show_context_menu')
})

test("Windows lets the webview open its own menu", async ({ launch }) => {
  const app = await launch({ platform: 'windows' })

  const prevented = preventedNextContextMenu(app)
  await app.editor.click({ button: 'right' })

  // WebView2 trims that menu on the Rust side, so the page neither cancels the
  // click nor asks for a menu of its own.
  expect(await prevented).toBe(false)
  expect(await app.commands()).not.toContain('show_context_menu')
})

test('the pane bar opens no menu at all', async ({ launch }) => {
  const app = await launch(MAC)

  const prevented = preventedNextContextMenu(app)
  await app.chars.click({ button: 'right' })

  // The webview's menu is stopped here as everywhere, but draftpad puts none
  // of its own in its place: there is nothing here to cut, copy or paste.
  expect(await prevented).toBe(true)
  expect(await app.commands()).not.toContain('show_context_menu')
})

test('a right click below the last line still asks for the menu', async ({ launch }) => {
  const app = await launch(MAC)

  await app.typeInEditor('一行だけ')
  // The blank space under a short draft is still the draft, and the menu has
  // to reach it the same way.
  const box = (await app.page.locator('#editor').boundingBox())!
  await app.page.mouse.click(box.x + 20, box.y + box.height - 20, { button: 'right' })

  await expect.poll(() => app.contextMenuState()).not.toBeNull()
})

test('the search panel offers the menu in its find field', async ({ launch }) => {
  const app = await launch(MAC)

  await app.runMenuCommand('find')
  await app.searchField.click({ button: 'right' })
  await expect.poll(() => app.contextMenuState()).not.toBeNull()
})

test('a fresh draft offers neither undo nor redo', async ({ launch }) => {
  const app = await launch(MAC)

  await app.editor.click({ button: 'right' })
  await expect.poll(() => app.contextMenuState()).toEqual({ canUndo: false, canRedo: false, canSearch: true })
})

test('the menu follows what the history holds', async ({ launch }) => {
  const app = await launch(MAC)

  await app.typeInEditor('書いた文字')
  await app.editor.click({ button: 'right' })
  await expect.poll(() => app.contextMenuState()).toEqual({ canUndo: true, canRedo: false, canSearch: true })

  await app.runMenuCommand('undo')
  await app.editor.click({ button: 'right' })
  await expect.poll(() => app.contextMenuState()).toEqual({ canUndo: false, canRedo: true, canSearch: true })
})

test('the menu leaves the draft alone while preferences has the keyboard', async ({ launch }) => {
  const app = await launch(MAC)

  await app.typeInEditor('残るはず')
  await app.gear.click()
  await expect(app.preferences).toBeVisible()

  // The same guards the command table carries: while the panel is open the
  // history and the search panel are not the draft's to touch, so the menu
  // must not offer them either.
  await app.page.locator('#pref-font-family').click({ button: 'right' })
  await expect.poll(() => app.contextMenuState()).toEqual({ canUndo: false, canRedo: false, canSearch: false })
})
