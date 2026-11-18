// The global the fake backend and the specs share through the page.

import type { Harness } from './backend'

declare global {
  interface Window {
    __pecha: Harness
  }
}
