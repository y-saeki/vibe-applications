// The two globals the fake backend and the specs share through the page.

import type { Harness } from './backend'

declare global {
  interface Window {
    __draftpad: Harness
    /** Installed by `mockIPC`; the specs use it to emit backend events. */
    __TAURI_INTERNALS__: { invoke: (cmd: string, args?: unknown) => Promise<unknown> }
  }
}
