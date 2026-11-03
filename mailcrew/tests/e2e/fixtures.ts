// Launches mailcrew's real frontend in a browser with the Rust side faked, and
// gives the specs the locators and queries they share.

import { readFileSync } from 'node:fs'

import { test as base, expect, type Locator, type Page } from '@playwright/test'

import type { Card, CardBody } from '../../src/backend'
import type { BackendConfig, Call } from './harness/backend'
import { HARNESS_BUNDLE } from './harness/paths'

export type LaunchOptions = Partial<BackendConfig> & {
  /** Serves the page under the Content-Security-Policy the built app runs with. */
  csp?: boolean
}

interface TauriConf {
  app: { security: { csp: Record<string, string> } }
}

/** The policy in src-tauri/tauri.conf.json, as a header value. */
function productionCsp(): string {
  const conf = JSON.parse(
    readFileSync(new URL('../../src-tauri/tauri.conf.json', import.meta.url), 'utf8'),
  ) as TauriConf
  return Object.entries(conf.app.security.csp)
    .map(([directive, value]) => `${directive} ${value}`)
    .join('; ')
}

const DEFAULT_CONFIG: BackendConfig = {
  email: 'me@example.com',
  platform: 'windows',
  inboxes: [[]],
  labels: [],
  failures: {},
  signInEmail: 'me@example.com',
}

/** A thread as the Rust side would hand it over; `n` keeps the ids apart. */
export function card(n: number, overrides: Partial<Card> = {}): Card {
  const body: CardBody = { kind: 'html', html: `<p>本文 ${n}</p>`, remoteImages: false }
  return {
    id: `t${n}`,
    subject: `件名 ${n}`,
    from: { name: `送信者 ${n}`, email: `sender${n}@example.com` },
    date: Date.UTC(2026, 0, 1) - n * 60_000,
    messageCount: 1,
    unread: false,
    starred: false,
    messageId: `m${n}`,
    messageIds: [`m${n}`],
    snippet: `スニペット ${n}`,
    body,
    attachments: [],
    ...overrides,
  }
}

export function cards(count: number): Card[] {
  return Array.from({ length: count }, (_, i) => card(i + 1))
}

export class App {
  readonly setup: Locator
  readonly strip: Locator
  readonly cards: Locator
  readonly empty: Locator
  readonly notice: Locator
  readonly count: Locator
  readonly doneAll: Locator
  readonly tagPicker: Locator
  readonly tagFilter: Locator

  constructor(
    readonly page: Page,
    /** Every Content-Security-Policy complaint the page has logged. */
    readonly cspViolations: string[],
  ) {
    this.setup = page.locator('#setup')
    this.strip = page.locator('#strip')
    this.cards = page.locator('.card[data-thread-id]')
    this.empty = page.locator('#empty')
    this.notice = page.locator('#notice')
    this.count = page.locator('#inbox-count')
    this.doneAll = page.locator('#done-all')
    this.tagPicker = page.locator('#tag-picker')
    this.tagFilter = page.locator('#tag-filter')
  }

  card(id: string): Locator {
    return this.page.locator(`.card[data-thread-id="${id}"]`)
  }

  tool(id: string, action: 'done' | 'tag' | 'later' | 'star'): Locator {
    return this.card(id).locator(`button[data-action="${action}"]`)
  }

  /** The iframe a card's HTML body is shown in. */
  frame(id: string): Locator {
    return this.card(id).locator('iframe.message-frame')
  }

  /** The ids of the cards on the strip, left to right. */
  ids(): Promise<string[]> {
    return this.cards.evaluateAll((elements) => elements.map((element) => (element as HTMLElement).dataset.threadId!))
  }

  calls(): Promise<Call[]> {
    return this.page.evaluate(() => window.__mailcrew.calls)
  }

  /** The calls made with one command, oldest first. */
  async callsTo(cmd: string): Promise<unknown[]> {
    return (await this.calls()).filter((call) => call.cmd === cmd).map((call) => call.args)
  }

  /** Waits for the number of calls to `cmd` to reach `count`. */
  async expectCalls(cmd: string, count: number): Promise<void> {
    await expect.poll(async () => (await this.callsTo(cmd)).length).toBe(count)
  }
}

export const test = base.extend<{ launch: (options?: LaunchOptions) => Promise<App> }>({
  launch: async ({ page }, use) => {
    await use(async (options: LaunchOptions = {}) => {
      const { csp, ...overrides } = options
      const config: BackendConfig = { ...DEFAULT_CONFIG, ...overrides }
      if (csp) {
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
        if (/content security policy|content-security-policy/i.test(message.text())) cspViolations.push(message.text())
      })
      await page.addInitScript((value: BackendConfig) => {
        window.__mailcrew = { config: value, calls: [] }
      }, config)
      await page.addInitScript({ path: HARNESS_BUNDLE })
      await page.goto('/')
      const app = new App(page, cspViolations)
      // The first thing the page does is ask who is signed in; the screen it
      // settles on follows from that.
      await expect.poll(() => app.callsTo('get_session').then((calls) => calls.length)).toBe(1)
      return app
    })
  },
})

export { expect }
