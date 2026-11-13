// Launches pecha-player's real frontend in a browser with the Rust side faked,
// and gives the specs the handful of locators and actions they all need.

import { readFileSync } from 'node:fs'

import { test as base, expect, type Locator, type Page } from '@playwright/test'

import type { BackendConfig, Call } from './harness/backend'
import { makePdf } from './harness/pdf'
import { HARNESS_BUNDLE } from './harness/paths'

export { expect }

export type LaunchOptions = Partial<BackendConfig> & {
  /**
   * Serves the page under the Content-Security-Policy the built app runs with.
   * The dev server sends none, so an asset the policy forbids loads here and
   * nowhere else.
   */
  csp?: boolean
}

/** The policy in src-tauri/tauri.conf.json, as a header value. */
function productionCsp(): string {
  const conf = JSON.parse(readFileSync(new URL('../../src-tauri/tauri.conf.json', import.meta.url), 'utf8')) as {
    app: { security: { csp: Record<string, string> } }
  }
  return Object.entries(conf.app.security.csp)
    .map(([directive, value]) => `${directive} ${value}`)
    .join('; ')
}

export class App {
  readonly root: Locator
  readonly loader: Locator
  readonly primary: Locator
  readonly loadButton: Locator
  readonly deckInfo: Locator
  readonly clock: Locator
  readonly counter: Locator
  readonly progress: Locator
  readonly shutter: Locator
  readonly endTitle: Locator
  readonly endDetail: Locator
  readonly error: Locator
  readonly signIn: Locator
  readonly signInButton: Locator
  readonly url: Locator

  constructor(
    readonly page: Page,
    readonly cspViolations: string[],
  ) {
    this.root = page.locator('#app')
    this.loader = page.locator('#loader')
    this.primary = page.locator('#primary')
    this.loadButton = page.locator('#load-button')
    this.deckInfo = page.locator('#deck-info')
    this.clock = page.locator('#clock')
    this.counter = page.locator('#counter')
    this.progress = page.locator('#progress')
    this.shutter = page.locator('#shutter')
    this.endTitle = page.locator('#end-title')
    this.endDetail = page.locator('#end-detail')
    this.error = page.locator('#error')
    this.signIn = page.locator('#sign-in')
    this.signInButton = page.locator('#sign-in-button')
    this.url = page.locator('#url')
  }

  /** Loads a PDF of `pages` pages as if the user had picked it. */
  async loadPdf(pages: number, name = 'talk.pdf'): Promise<void> {
    await this.page.locator('#file-input').setInputFiles({
      name,
      mimeType: 'application/pdf',
      buffer: Buffer.from(makePdf(pages)),
    })
  }

  async loadUrl(url: string): Promise<void> {
    await this.url.fill(url)
    await this.page.locator('#open-url').click()
  }

  /**
   * Waits until page `n` of the PDF is the one on screen.
   *
   * PDF.js runs part of its work on `setTimeout` — it waits 100 ms before
   * dropping a cancelled page drawing, and a new drawing of that page waits
   * with it — and a paused fake clock holds those timers back. Each poll that
   * does not find the page yet moves the clock on by 50 ms, which keeps the
   * cases' whole-second checks well inside their second.
   */
  async expectPage(n: number): Promise<void> {
    await expect
      .poll(async () => {
        const page = await this.root.getAttribute('data-page')
        if (page !== String(n)) await this.page.clock.runFor(50)
        return page
      })
      .toBe(String(n))
  }

  async expectPhase(phase: 'empty' | 'ready' | 'running' | 'closed'): Promise<void> {
    await expect(this.root).toHaveAttribute('data-phase', phase)
  }

  async calls(): Promise<Call[]> {
    return this.page.evaluate(() => window.__pecha.calls)
  }

  async emit(event: string, payload: unknown): Promise<void> {
    await this.page.evaluate(([e, p]) => window.__pecha.emit(e as string, p), [event, payload] as const)
  }
}

export const test = base.extend<{ launch: (options?: LaunchOptions) => Promise<App> }>({
  launch: async ({ page }, use) => {
    await use(async (options = {}) => {
      const config: BackendConfig = { google: options.google ?? {} }
      if (options.csp) {
        const policy = productionCsp()
        await page.route(
          (url) => url.pathname === '/',
          async (route) => {
            const response = await route.fetch()
            await route.fulfill({ response, headers: { ...response.headers(), 'content-security-policy': policy } })
          },
        )
      }
      const cspViolations: string[] = []
      page.on('console', (message) => {
        if (/content security policy/i.test(message.text())) cspViolations.push(message.text())
      })
      await page.clock.install()
      await page.addInitScript((value: BackendConfig) => {
        window.__pecha = { config: value, calls: [], emit: async () => {} }
      }, config)
      await page.addInitScript({ path: HARNESS_BUNDLE })
      await page.goto('/')
      const app = new App(page, cspViolations)
      await app.expectPhase('empty')
      return app
    })
  },
})
