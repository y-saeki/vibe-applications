// A right click opens draftpad's own menu (src-tauri/src/menu.rs) rather than
// the webview's. The menu itself is native, so what can be checked here is the
// half that lives in the page: that the webview's menu is stopped, and what the
// page tells the Rust side to enable in its place.

import { type App, expect, test } from './fixtures'

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

for (const platform of ['macos', 'windows']) {
  test(`the webview's own menu never opens on ${platform}`, async ({ launch }) => {
    const app = await launch({ platform })

    const prevented = preventedNextContextMenu(app)
    await app.editor.click({ button: 'right' })

    expect(await prevented).toBe(true)
    await expect.poll(() => app.commands()).toContain('show_context_menu')
  })
}

test('a right click anywhere in the window asks for the menu', async ({ launch }) => {
  const app = await launch()

  // The status bar is no part of the draft, but a right click there is still a
  // right click on draftpad.
  await app.chars.click({ button: 'right' })
  await expect.poll(() => app.contextMenuState()).not.toBeNull()
})

test('a fresh draft offers neither undo nor redo', async ({ launch }) => {
  const app = await launch()

  await app.editor.click({ button: 'right' })
  await expect.poll(() => app.contextMenuState()).toEqual({ canUndo: false, canRedo: false, canSearch: true })
})

test('the menu follows what the history holds', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('書いた文字')
  await app.editor.click({ button: 'right' })
  await expect.poll(() => app.contextMenuState()).toEqual({ canUndo: true, canRedo: false, canSearch: true })

  await app.runMenuCommand('undo')
  await app.editor.click({ button: 'right' })
  await expect.poll(() => app.contextMenuState()).toEqual({ canUndo: false, canRedo: true, canSearch: true })
})

test('the menu leaves the draft alone while preferences has the keyboard', async ({ launch }) => {
  const app = await launch()

  await app.typeInEditor('残るはず')
  await app.gear.click()
  await expect(app.preferences).toBeVisible()

  // The same guards the command table carries: while the panel is open the
  // history and the search panel are not the draft's to touch, so the menu
  // must not offer them either.
  await app.preferences.click({ button: 'right' })
  await expect.poll(() => app.contextMenuState()).toEqual({ canUndo: false, canRedo: false, canSearch: false })
})
