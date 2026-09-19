// Launches diffpad's real frontend in a browser with the Rust side faked, and
// gives the specs the handful of locators and queries they all need.

import { readFileSync } from 'node:fs'

import { test as base, expect, type Locator, type Page } from '@playwright/test'

import type { BackendConfig, Call, State } from './harness/backend'
import { HARNESS_BUNDLE } from './harness/paths'

export type { State }

/** The left pane is `a` and the right one `b`, as src/editor.ts names them. */
export type Side = 'a' | 'b'

/** Everything a spec may vary about the fake backend; the rest is filled in. */
export type LaunchOptions = Partial<BackendConfig> & {
  /** What the OS reports for `prefers-color-scheme`. */
  colorScheme?: 'light' | 'dark'
  /**
   * Serves the page under the Content-Security-Policy the built app runs with.
   * The dev server sends none, so an asset the policy forbids loads here and
   * nowhere else.
   */
  csp?: boolean
}

/** The policy in src-tauri/tauri.conf.json, as a header value. */
function productionCsp(): string {
  const path = new URL('../../src-tauri/tauri.conf.json', import.meta.url)
  const conf = JSON.parse(readFileSync(path, 'utf8')) as {
    app: { security: { csp: Record<string, string> } }
  }
  return Object.entries(conf.app.security.csp)
    .map(([directive, value]) => `${directive} ${value}`)
    .join('; ')
}

const DEFAULT_CONFIG: BackendConfig = {
  platform: 'windows',
  version: '0.0.0-test',
  fonts: [],
  innerSize: { width: 960, height: 560 },
  scaleFactor: 1,
}

/** The debounce in src/state.ts, with room for the write itself. */
const SAVE_TIMEOUT_MS = 3000

export class App {
  /** The two panes' editable content. */
  readonly editorA: Locator
  readonly editorB: Locator
  readonly diffCount: Locator
  readonly previousDiff: Locator
  readonly nextDiff: Locator
  readonly diffModeSelect: Locator
  readonly alwaysOnTop: Locator
  readonly gear: Locator
  readonly preferences: Locator

  constructor(
    readonly page: Page,
    readonly platform: string,
    /** Every Content-Security-Policy complaint the page has logged. */
    readonly cspViolations: string[],
  ) {
    this.editorA = page.locator('.cm-merge-a .cm-content')
    this.editorB = page.locator('.cm-merge-b .cm-content')
    this.diffCount = page.locator('#status-diff')
    this.previousDiff = page.locator('#previous-diff')
    this.nextDiff = page.locator('#next-diff')
    this.diffModeSelect = page.locator('#diff-mode-select')
    this.alwaysOnTop = page.locator('#always-on-top')
    this.gear = page.locator('#open-preferences')
    this.preferences = page.locator('#preferences')
  }

  editor(side: Side): Locator {
    return side === 'a' ? this.editorA : this.editorB
  }

  /** The pane's whole editor element, which is what carries the merge view's side class. */
  pane(side: Side): Locator {
    return this.page.locator(`.cm-merge-${side}`)
  }

  /** The lines the merge view has marked as part of a chunk, on one side. */
  changedLines(side: Side): Locator {
    return this.pane(side).locator('.cm-changedLine')
  }

  /** The characters the merge view has marked as differing, on one side. */
  changedText(side: Side): Locator {
    return this.pane(side).locator('.cm-changedText')
  }

  /** The gaps the merge view has opened in one pane, opposite lines only the other pane has. */
  gaps(side: Side): Locator {
    return this.pane(side).locator('.cm-mergeSpacer')
  }

  /** "Mod" from src/commands.ts: Cmd on macOS, Ctrl elsewhere. */
  get mod(): 'Meta' | 'Control' {
    return this.platform === 'macos' ? 'Meta' : 'Control'
  }

  press(key: string): Promise<void> {
    return this.page.keyboard.press(`${this.mod}+${key}`)
  }

  /** Every `invoke` the app has made so far, oldest first. */
  calls(): Promise<Call[]> {
    return this.page.evaluate(() => window.__diffpad.calls)
  }

  commands(): Promise<string[]> {
    return this.page.evaluate(() => window.__diffpad.calls.map((call) => call.cmd))
  }

  /** The state handed to the most recent `save_state`, or null before the first. */
  saved(): Promise<State | null> {
    return this.page.evaluate(() => window.__diffpad.saved)
  }

  /** Waits for a `save_state` whose state satisfies `predicate`. */
  async expectSaved(predicate: (state: State) => boolean): Promise<void> {
    await expect
      .poll(
        async () => {
          const saved = await this.saved()
          return saved !== null && predicate(saved)
        },
        { timeout: SAVE_TIMEOUT_MS },
      )
      .toBe(true)
  }

  /**
   * True when a select's dropped-open list is one the page styles rather than
   * the platform's own window. That needs both an engine that can do it and
   * the platform diffpad asks for it on, so it reads the result rather than
   * either half: `CSS.supports` alone would say yes on a Mac.
   */
  listIsOurs(): Promise<boolean> {
    return this.page.evaluate(
      () => getComputedStyle(document.querySelector('#diff-mode-select')!).appearance === 'base-select',
    )
  }

  /** Delivers a "menu" event, the way src-tauri/src/menu.rs does on macOS. */
  async runMenuCommand(id: string): Promise<void> {
    await this.page.evaluate(
      (payload) => window.__TAURI_INTERNALS__.invoke('plugin:event|emit', { event: 'menu', payload }),
      id,
    )
  }

  /**
   * The line the caret of `side` is on, 1-based, or 0 when the caret is not in
   * that pane. CodeMirror keeps the native selection inside the line the caret
   * is on, whichever pane draws it, so the DOM selection is what says.
   */
  caretLine(side: Side): Promise<number> {
    return this.page.evaluate((selector) => {
      const node = document.getSelection()?.focusNode
      const element = node instanceof Element ? node : node?.parentElement
      const line = element?.closest('.cm-line')
      const lines = Array.from(document.querySelector(selector)!.querySelectorAll('.cm-line'))
      return line ? lines.indexOf(line) + 1 : 0
    }, `.cm-merge-${side} .cm-content`)
  }

  /**
   * Puts `text` into a pane, one insertion per line rather than one event per
   * character — which is also what happens when an IME commits a phrase.
   *
   * Character-by-character typing races with CodeMirror in WebKit: it re-syncs
   * the DOM after the first character of an empty line, the caret can come back
   * at offset 0, and everything typed next lands in front of it. A single
   * insertion has nothing to interleave with.
   *
   * Newlines are pressed, so they still go through the keymap. The pane is
   * clicked first unless it already has the focus, so the text lands where it
   * was meant to.
   */
  async typeInEditor(side: Side, text: string): Promise<void> {
    const editor = this.editor(side)
    if (!(await editor.evaluate((element) => element === document.activeElement))) await editor.click()
    await expect(editor).toBeFocused()
    for (const [index, line] of text.split('\n').entries()) {
      if (index > 0) await this.page.keyboard.press('Enter')
      if (line) await this.page.keyboard.insertText(line)
    }
  }
}

export const test = base.extend<{ launch: (options?: LaunchOptions) => Promise<App> }>({
  launch: async ({ page }, use) => {
    await use(async (options: LaunchOptions = {}) => {
      const { colorScheme, csp, ...overrides } = options
      const config: BackendConfig = { ...DEFAULT_CONFIG, ...overrides }
      if (colorScheme) await page.emulateMedia({ colorScheme })
      if (csp) {
        const policy = productionCsp()
        await page.route((url) => url.pathname === '/', async (route) => {
          const response = await route.fetch()
          await route.fulfill({
            response,
            headers: { ...response.headers(), 'content-security-policy': policy },
          })
        })
      }
      const cspViolations: string[] = []
      page.on('console', (message) => {
        if (/content security policy/i.test(message.text())) cspViolations.push(message.text())
      })
      await page.addInitScript((value: BackendConfig) => {
        window.__diffpad = { config: value, calls: [], saved: null }
      }, config)
      await page.addInitScript({ path: HARNESS_BUNDLE })
      await page.goto('/')
      const app = new App(page, config.platform, cspViolations)
      // main() shows the window as its very last step, so this is the signal
      // that the whole frontend is wired up.
      await expect.poll(() => app.commands(), { timeout: 15000 }).toContain('plugin:window|show')
      return app
    })
  },
})

export { expect }
