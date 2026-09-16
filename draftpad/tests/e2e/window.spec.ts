import { expect } from '@playwright/test'

import { test } from './fixtures'

test('lays a patch under the traffic lights while the window is inactive', async ({ launch }) => {
  const app = await launch({ platform: 'macos', colorScheme: 'dark' })
  const html = app.page.locator('html')
  const background = (): Promise<string> =>
    app.page.evaluate(() => getComputedStyle(document.querySelector('#titlebar')!).backgroundImage)

  await expect(html).toHaveAttribute('data-window', 'active')
  expect(await background()).toBe('none')

  await app.page.evaluate(() => window.dispatchEvent(new Event('blur')))
  await expect(html).toHaveAttribute('data-window', 'inactive')
  expect(await background()).not.toBe('none')

  await app.page.evaluate(() => window.dispatchEvent(new Event('focus')))
  await expect(html).toHaveAttribute('data-window', 'active')
  expect(await background()).toBe('none')
})

test('keeps the strip out of the way on Windows, which has its own title bar', async ({ launch }) => {
  const app = await launch({ platform: 'windows', colorScheme: 'dark' })

  await app.page.evaluate(() => window.dispatchEvent(new Event('blur')))
  await expect(app.page.locator('html')).toHaveAttribute('data-window', 'inactive')
  await expect(app.page.locator('#titlebar')).toHaveCSS('height', '0px')
})
