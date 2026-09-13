// Launches draftpad's real frontend in a browser with the Rust side faked, and
// gives the specs the handful of locators and queries they all need.

import { test as base, expect, type Locator, type Page } from '@playwright/test'

import type { BackendConfig, Call, State } from './harness/backend'
import { HARNESS_BUNDLE } from './harness/paths'

export type { State }

/** Everything a spec may vary about the fake backend; the rest is filled in. */
export type LaunchOptions = Partial<BackendConfig> & {
  /** What the OS reports for `prefers-color-scheme`. */
  colorScheme?: 'light' | 'dark'
}

const DEFAULT_CONFIG: BackendConfig = {
  platform: 'windows',
  version: '0.0.0-test',
  openPreferences: false,
  fonts: [],
  innerSize: { width: 600, height: 400 },
  scaleFactor: 1,
}

/** The debounce in src/state.ts, with room for the write itself. */
const SAVE_TIMEOUT_MS = 3000

export class App {
  readonly editor: Locator
  readonly chars: Locator
  readonly lines: Locator
  readonly languageSelect: Locator
  readonly alwaysOnTop: Locator
  readonly gear: Locator
  readonly preferences: Locator
  readonly searchPanel: Locator

  constructor(
    readonly page: Page,
    readonly platform: string,
  ) {
    this.editor = page.locator('.cm-content')
    this.chars = page.locator('#status-chars')
    this.lines = page.locator('#status-lines')
    this.languageSelect = page.locator('#language-select')
    this.alwaysOnTop = page.locator('#always-on-top')
    this.gear = page.locator('#open-preferences')
    this.preferences = page.locator('#preferences')
    this.searchPanel = page.locator('.cm-search')
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
    return this.page.evaluate(() => window.__draftpad.calls)
  }

  commands(): Promise<string[]> {
    return this.page.evaluate(() => window.__draftpad.calls.map((call) => call.cmd))
  }

  /** The state handed to the most recent `save_state`, or null before the first. */
  saved(): Promise<State | null> {
    return this.page.evaluate(() => window.__draftpad.saved)
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

  /** Delivers a "menu" event, the way src-tauri/src/menu.rs does on macOS. */
  async runMenuCommand(id: string): Promise<void> {
    await this.page.evaluate(
      (payload) => window.__TAURI_INTERNALS__.invoke('plugin:event|emit', { event: 'menu', payload }),
      id,
    )
  }

  /**
   * Puts `text` into the editor, one insertion per line rather than one event
   * per character — which is also what happens when an IME commits a phrase.
   *
   * Character-by-character typing races with CodeMirror in WebKit: it re-syncs
   * the DOM after the first character of an empty line, the caret can come back
   * at offset 0, and everything typed next lands in front of it, so
   * "閉じる前に残す" arrives as "じる前に残す閉". A single insertion has nothing
   * to interleave with.
   *
   * Newlines are pressed, so they still go through the keymap. There is no
   * click: draftpad focuses the editor itself, at startup and whenever the
   * preferences panel closes, and asserting that makes the precondition checked
   * rather than assumed.
   */
  async typeInEditor(text: string): Promise<void> {
    await expect(this.editor).toBeFocused()
    for (const [index, line] of text.split('\n').entries()) {
      if (index > 0) await this.page.keyboard.press('Enter')
      if (line) await this.page.keyboard.insertText(line)
    }
  }
}

export const test = base.extend<{ launch: (options?: LaunchOptions) => Promise<App> }>({
  launch: async ({ page }, use) => {
    await use(async (options: LaunchOptions = {}) => {
      const { colorScheme, ...overrides } = options
      const config: BackendConfig = { ...DEFAULT_CONFIG, ...overrides }
      if (colorScheme) await page.emulateMedia({ colorScheme })
      await page.addInitScript((value: BackendConfig) => {
        window.__draftpad = { config: value, calls: [], saved: null }
      }, config)
      await page.addInitScript({ path: HARNESS_BUNDLE })
      await page.goto('/')
      const app = new App(page, config.platform)
      // main() shows the window as its very last step, so this is the signal
      // that the whole frontend is wired up.
      await expect.poll(() => app.commands(), { timeout: 15000 }).toContain('plugin:window|show')
      return app
    })
  },
})

export { expect }
