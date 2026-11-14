// Everything the frontend asks of the Rust side (src-tauri/src/lib.rs).

import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

/** Mirrors `DeckError` in src-tauri/src/google.rs. */
export type RemoteError =
  | { kind: 'invalidUrl' }
  | { kind: 'signInRequired' }
  | { kind: 'forbidden' }
  | { kind: 'notFound' }
  | { kind: 'tooLarge' }
  | { kind: 'notPdf' }
  | { kind: 'network'; detail: string }

export function isRemoteError(value: unknown): value is RemoteError {
  return typeof value === 'object' && value !== null && typeof (value as { kind?: unknown }).kind === 'string'
}

/** Downloads the Google Slides deck at `url` as PDF bytes. Rejects with a `RemoteError`. */
export async function loadGoogleSlides(url: string): Promise<Uint8Array> {
  const bytes = await invoke<ArrayBuffer>('load_google_slides', { url })
  return new Uint8Array(bytes)
}

/** Opens Google's sign-in in a window of its own, continuing to the deck at `url`. */
export async function openGoogleSignIn(url: string): Promise<void> {
  await invoke('open_google_sign_in', { url })
}

/** Called with `done` once the sign-in window reached the deck, `closed` if it was closed first. */
export function onGoogleSignIn(handler: (result: 'done' | 'closed') => void): void {
  void listen<'done' | 'closed'>('google-sign-in', (event) => handler(event.payload))
}

/** The ordinary fullscreen; on macOS, the one the green title bar button enters. */
export async function setFullscreen(on: boolean): Promise<void> {
  await getCurrentWindow().setFullscreen(on)
}

/**
 * Lets the slide fill the whole screen: on macOS with no title bar, menu bar or
 * Dock (src-tauri/src/fullscreen.rs), elsewhere the ordinary fullscreen.
 */
export async function setSlideFullscreen(on: boolean): Promise<void> {
  await invoke('set_slide_fullscreen', { on })
}

export async function setTitle(title: string): Promise<void> {
  await getCurrentWindow().setTitle(title)
}

export async function isFullscreen(): Promise<boolean> {
  return getCurrentWindow().isFullscreen()
}

export function onResized(handler: () => void): void {
  void getCurrentWindow().onResized(handler)
}
