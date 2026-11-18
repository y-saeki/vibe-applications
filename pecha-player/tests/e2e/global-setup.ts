// Bundles the fake backend once per run. It has to reach the page as a plain
// script, so the imports in tests/e2e/harness/backend.ts are resolved here
// rather than in the browser.

import * as esbuild from 'esbuild'

import { HARNESS_BUNDLE, HARNESS_ENTRY } from './harness/paths'

export default async function globalSetup(): Promise<void> {
  await esbuild.build({
    entryPoints: [HARNESS_ENTRY],
    outfile: HARNESS_BUNDLE,
    bundle: true,
    format: 'iife',
    target: 'es2020',
    logLevel: 'warning',
  })
}
