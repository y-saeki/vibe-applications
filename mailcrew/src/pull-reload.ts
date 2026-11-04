// Pull to reload: a zone before the first card that the strip normally keeps
// scrolled out of sight. Pulling the strip right from its first card draws the
// zone out, and letting go once it is all the way out reloads.
//
// The zone is real scrollable space, so the strip moves under the fingers the
// way it always does. What makes a movement a pull is who is moving it: only a
// gesture that began while the strip was at rest on its first card may pull.
// Momentum that carries a fling back to the start never does; the strip just
// springs back to the first card.
//
// Where a gesture begins and ends is known two ways:
//
// - On macOS the Rust side reports fingers touching and leaving the trackpad
//   (`src-tauri/src/trackpad.rs`), straight from AppKit, over a message's frame
//   as much as anywhere else.
// - Elsewhere the page has only wheel events, which say nothing of fingers or
//   momentum. A run of them that starts after a pause is a gesture, and it ends
//   when they stop: momentum follows the fingers without a pause, so it stays
//   part of the gesture that flung it.

import { listen } from '@tauri-apps/api/event'

import { ICONS } from './icons'

/** How long the strip has to be still before it counts as at rest. */
const SETTLE_MS = 150
/** The pause in wheel events that makes the next one start a new gesture. */
const WHEEL_GAP_MS = 200
/** How long wheel events have to stop for a wheel gesture to count as let go. */
const WHEEL_RELEASE_MS = 140

const LABEL_PULL = '引っ張って再読み込み'
const LABEL_READY = '離すと再読み込み'
const LABEL_BUSY = '読み込み中'

/** What the gesture in progress may do. */
type Gesture = 'none' | 'pull' | 'scroll'

export class PullToReload {
  readonly zone: HTMLElement
  private readonly icon: HTMLElement
  private readonly label: HTMLElement
  /**
   * Set while the strip is still at the first card, so a gesture starting now
   * may pull. The zone's `data-armed` says the same, for the tests to wait on.
   */
  private restingNow = true
  /** `pull` for a gesture that began at rest, `scroll` for any other. */
  private gesture: Gesture = 'none'
  private pulling = false
  /** Set while the zone is all the way out, so letting go now reloads. */
  private ready = false
  private busy = false
  private settleTimer: number | undefined
  private wheelTimer: number | undefined
  private lastWheel = -Infinity

  constructor(
    private readonly strip: HTMLElement,
    private readonly onPull: () => Promise<void>,
  ) {
    this.zone = document.createElement('div')
    this.zone.className = 'pull-zone'
    this.zone.setAttribute('aria-hidden', 'true')
    this.icon = document.createElement('span')
    this.icon.className = 'pull-icon'
    this.icon.innerHTML = ICONS.reload
    this.label = document.createElement('span')
    this.label.className = 'pull-label'
    this.label.textContent = LABEL_PULL
    this.zone.append(this.icon, this.label)
    this.resting = true
    strip.addEventListener('scroll', () => this.onScroll(), { passive: true })
    strip.addEventListener('wheel', () => this.wheel(), { passive: true })
    void listen<string>('trackpad', (event) => {
      if (event.payload === 'down') this.fingersDown()
      else if (event.payload === 'up') this.fingersUp()
    })
    // The strip has no width while it is hidden; put it back once it has one,
    // or when the window's width changes. A change of height alone leaves the
    // strip where it is, so it cannot undo a pull that has just begun.
    let width = -1
    new ResizeObserver(() => {
      const now = this.strip.clientWidth
      if (now === width) return
      width = now
      if (this.resting && !this.pulling) this.strip.scrollLeft = this.rest
    }).observe(strip)
  }

  private get resting(): boolean {
    return this.restingNow
  }

  private set resting(value: boolean) {
    this.restingNow = value
    this.zone.dataset.armed = String(value && !this.busy)
    // Scroll snapping would carry a pull straight back to the first card
    // (WebKit snaps before the strip reaches the start), so it is off while
    // the strip is at the first card, and comes back once the strip settles
    // further along.
    if (value) this.strip.classList.add('strip-free')
  }

  /** On macOS gestures come from the trackpad itself; wheel events are not needed. */
  private get fingersReported(): boolean {
    return document.documentElement.dataset.platform === 'macos'
  }

  /** The scroll position that shows the first card with the zone out of sight. */
  get rest(): number {
    // Layout positions, since the cards are shifted while they lag behind a pull.
    const first = this.zone.nextElementSibling as HTMLElement | null
    if (!first || !this.zone.isConnected) return 0
    return first.offsetLeft - this.zone.offsetLeft
  }

  /** Scrolls back to the first card, as after the strip is laid out again. */
  reset(): void {
    this.pulling = false
    this.ready = false
    this.resting = true
    this.strip.scrollLeft = this.rest
    this.draw(0)
  }

  /**
   * A wheel event over the strip, or over a message's frame, whose events do
   * not reach the strip. Ignored where the fingers are reported.
   */
  wheel(): void {
    if (this.fingersReported) return
    const now = performance.now()
    if (now - this.lastWheel >= WHEEL_GAP_MS) this.begin()
    this.lastWheel = now
    window.clearTimeout(this.wheelTimer)
    this.wheelTimer = window.setTimeout(() => this.end(), WHEEL_RELEASE_MS)
  }

  private fingersDown(): void {
    if (this.gesture === 'none') this.begin()
  }

  private fingersUp(): void {
    this.end()
  }

  private begin(): void {
    this.gesture = this.resting && !this.busy ? 'pull' : 'scroll'
  }

  /** The gesture is over: reloads if the zone is all the way out, or puts it away. */
  private end(): void {
    const pulled = this.gesture === 'pull'
    this.gesture = 'none'
    this.pulling = false
    if (pulled && this.ready && this.strip.scrollLeft <= 1) {
      void this.fire()
      return
    }
    this.ready = false
    this.draw(0)
    this.putAway()
  }

  /** Scrolls a zone that is partly out back out of sight. */
  private putAway(): void {
    if (this.busy || this.gesture === 'pull') return
    if (this.strip.scrollLeft < this.rest - 1) this.strip.scrollTo({ left: this.rest, behavior: 'smooth' })
  }

  private onScroll(): void {
    const rest = this.rest
    const left = this.strip.scrollLeft
    if (left >= rest - 1) {
      this.pulling = false
      if (left > rest + 1) this.resting = false
    } else {
      this.pulling = this.gesture === 'pull' && !this.busy
    }
    this.ready = this.pulling && left <= 1
    this.draw(this.pulling ? 1 - left / rest : 0)
    window.clearTimeout(this.settleTimer)
    this.settleTimer = window.setTimeout(() => this.settle(), SETTLE_MS)
  }

  /**
   * Once the strip stops: remembers that it is at the first card, lets it snap
   * again further along, or springs it back when something other than a pull,
   * such as momentum, carried it past the first card.
   */
  private settle(): void {
    const rest = this.rest
    const left = this.strip.scrollLeft
    if (Math.abs(left - rest) <= 1) this.resting = true
    else if (left > rest) this.strip.classList.remove('strip-free')
    else this.putAway()
  }

  private async fire(): Promise<void> {
    this.pulling = false
    this.ready = false
    this.resting = false
    this.busy = true
    this.zone.classList.add('pull-busy')
    this.label.textContent = LABEL_BUSY
    this.zone.style.opacity = '1'
    try {
      await this.onPull()
    } finally {
      this.busy = false
      this.resting = this.restingNow
      this.zone.classList.remove('pull-busy')
      this.draw(0)
      this.putAway()
    }
  }

  private draw(progress: number): void {
    const shown = Math.max(0, Math.min(1, progress))
    this.zone.classList.toggle('pull-ready', this.ready)
    if (this.busy) return
    this.label.textContent = this.ready ? LABEL_READY : LABEL_PULL
    this.zone.style.opacity = String(shown)
    this.icon.style.transform = `rotate(${shown * 270}deg)`
  }
}
