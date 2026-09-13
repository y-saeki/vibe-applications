// Where the bundled fake backend lives. Shared by the global setup that writes
// it and the fixture that injects it.

import { fileURLToPath } from 'node:url'

const here = fileURLToPath(new URL('.', import.meta.url))

export const HARNESS_ENTRY = `${here}backend.ts`
export const HARNESS_BUNDLE = `${here}dist/backend.js`
