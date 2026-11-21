// The built app runs under the Content-Security-Policy in tauri.conf.json, the
// dev server under none. PDF.js loads a worker, wasm and data files of its
// own, so this is where a policy that forbids one of them shows up.

import { expect, test } from './fixtures'

test('a deck opens and plays under the production policy', async ({ launch }) => {
  const app = await launch({ csp: true })
  await app.loadPdf(21)
  await app.expectPhase('ready')
  await app.expectPage(1)
  await app.primary.click()
  await app.expectPage(2)
  expect(app.cspViolations).toEqual([])
})
