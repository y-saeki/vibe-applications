// Pull to reload: a zone before the first card that the strip normally keeps
// scrolled out of sight. Swiping right at the start of the strip draws it out,
// and drawing it out all the way reloads.
//
// The zone is real scrollable space rather than a reading of wheel deltas, so
// the gesture works wherever the strip scrolls natively: over a message's
// frame too, whose events never reach the page in WKWebView.

import { ICONS } from './icons'

/**
 * How long the strip has to be still before a swipe counts as a new gesture.
 * Momentum that carries a fling past the first card arrives sooner than this,
 * so it never reloads by itself.
 */
const SETTLE_MS = 150

export class PullToReload {
  readonly zone: HTMLElement
  private readonly icon: HTMLElement
  /**
   * Set while the strip is still at the first card, so a swipe from there may
   * pull. The zone's `data-armed` says the same, for the tests to wait on.
   */
  private restingNow = true
  private pulling = false
  private busy = false
  private settleTimer: number | undefined

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
    this.zone.append(this.icon)
    this.resting = true
    strip.addEventListener('scroll', () => this.onScroll(), { passive: true })
    // The strip has no width while it is hidden; put it back once it has one.
    new ResizeObserver(() => {
      if (this.resting && !this.pulling) this.strip.scrollLeft = this.rest
    }).observe(strip)
  }

  private get resting(): boolean {
    return this.restingNow
  }

  private set resting(value: boolean) {
    this.restingNow = value
    this.zone.dataset.armed = String(value && !this.busy)
  }

  /** The scroll position that shows the first card with the zone out of sight. */
  get rest(): number {
    const first = this.zone.nextElementSibling
    if (!first || !this.zone.isConnected) return 0
    return first.getBoundingClientRect().left - this.zone.getBoundingClientRect().left
  }

  /** Scrolls back to the first card, as after the strip is laid out again. */
  reset(): void {
    this.pulling = false
    this.resting = true
    this.strip.scrollLeft = this.rest
    this.draw(0)
  }

  private onScroll(): void {
    const rest = this.rest
    const left = this.strip.scrollLeft
    if (left >= rest - 1) {
      this.pulling = false
      if (left > rest + 1) this.resting = false
    } else if (this.resting && !this.busy) {
      this.pulling = true
    }
    this.draw(this.pulling ? 1 - left / rest : 0)
    if (this.pulling && left <= 1) void this.fire()
    window.clearTimeout(this.settleTimer)
    this.settleTimer = window.setTimeout(() => this.settle(), SETTLE_MS)
  }

  /** Once the strip stops: remembers it is at the start, or puts a half-drawn zone away. */
  private settle(): void {
    const rest = this.rest
    const left = this.strip.scrollLeft
    if (Math.abs(left - rest) <= 1) this.resting = true
    else if (left < rest && !this.busy) this.strip.scrollTo({ left: rest, behavior: 'smooth' })
  }

  private async fire(): Promise<void> {
    this.pulling = false
    this.resting = false
    this.busy = true
    this.zone.classList.add('pull-busy')
    try {
      await this.onPull()
    } finally {
      this.busy = false
      this.resting = this.restingNow
      this.zone.classList.remove('pull-busy')
      this.draw(0)
      if (this.strip.scrollLeft < this.rest) this.strip.scrollTo({ left: this.rest, behavior: 'smooth' })
    }
  }

  private draw(progress: number): void {
    const shown = Math.max(0, Math.min(1, progress))
    this.zone.classList.toggle('pull-ready', shown >= 1)
    this.icon.style.opacity = this.busy ? '1' : String(shown)
    if (!this.busy) this.icon.style.transform = `rotate(${shown * 270}deg)`
  }
}
