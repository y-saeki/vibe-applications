// End-to-end tests for the frontend. The Rust side is faked
// (tests/e2e/harness/), so the whole suite runs on one Linux runner while still
// exercising src/ exactly as shipped.
//
// The two projects are the two webview engines draftpad ships on: Chromium
// stands in for WebView2 on Windows, WebKit for WKWebView on macOS.

import { defineConfig } from '@playwright/test'

const PORT = 1420
const BASE_URL = `http://127.0.0.1:${PORT}`

export default defineConfig({
  testDir: './tests/e2e',
  globalSetup: './tests/e2e/global-setup.ts',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['github'], ['html', { open: 'never' }]] : [['list']],
  use: {
    baseURL: BASE_URL,
    trace: 'on-first-retry',
  },
  projects: [
    { name: 'chromium', use: { browserName: 'chromium' } },
    { name: 'webkit', use: { browserName: 'webkit' } },
  ],
  webServer: {
    command: 'node build.mjs --serve',
    url: BASE_URL,
    reuseExistingServer: !process.env.CI,
  },
})
