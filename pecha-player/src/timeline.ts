// The PechaKucha clock, as a pure function of time.
//
// Everything on screen during a run — which slide shows, the counter, the
// elapsed clock, the progress bar, the moment the shutter closes — is derived
// from the instant Start was pressed and the instant now. Nothing accumulates
// tick by tick, so a late timer (a busy machine, a throttled window) can only
// make the screen catch up, never drift.

/** How long each slide stays up. */
export const SLIDE_MS = 20_000

/** How many slides a PechaKucha talk has. */
export const TALK_SLIDES = 20

/**
 * How many slides a deck of `pageCount` pages plays: the first page is the
 * cover that waits before Start, and at most 20 pages follow it.
 */
export function playableSlides(pageCount: number): number {
  return Math.max(0, Math.min(TALK_SLIDES, pageCount - 1))
}

export interface Moment {
  /** 1-based slide number since Start (the counter); the page shown is `slide + 1`. */
  slide: number
  /** Milliseconds since Start, capped at the end of the talk. */
  elapsedMs: number
  /** Milliseconds the current slide has been up. */
  slideElapsedMs: number
  /** Whether the last slide has had its 20 seconds. */
  finished: boolean
}

/** Where a run that started at `startedAt` stands at `now`. */
export function momentAt(startedAt: number, now: number, slides: number): Moment {
  const total = slides * SLIDE_MS
  const elapsed = Math.max(0, now - startedAt)
  if (elapsed >= total) {
    return { slide: slides, elapsedMs: total, slideElapsedMs: SLIDE_MS, finished: true }
  }
  const index = Math.floor(elapsed / SLIDE_MS)
  return {
    slide: index + 1,
    elapsedMs: elapsed,
    slideElapsedMs: elapsed - index * SLIDE_MS,
    finished: false,
  }
}

/**
 * The next instant after `now` at which something on screen changes: the
 * clock's next whole second or the next flip, whichever comes first.
 */
export function nextChangeAt(startedAt: number, now: number, slides: number): number {
  const elapsed = Math.max(0, now - startedAt)
  const total = slides * SLIDE_MS
  const nextSecond = (Math.floor(elapsed / 1000) + 1) * 1000
  const nextFlip = (Math.floor(elapsed / SLIDE_MS) + 1) * SLIDE_MS
  return startedAt + Math.min(nextSecond, nextFlip, total)
}

/** `m:ss`, the way the elapsed clock reads. */
export function formatClock(ms: number): string {
  const seconds = Math.floor(ms / 1000)
  const m = Math.floor(seconds / 60)
  const s = seconds % 60
  return `${m}:${String(s).padStart(2, '0')}`
}
