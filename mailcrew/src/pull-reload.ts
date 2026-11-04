// Pull to reload: a zone before the first card that the strip normally keeps
// scrolled out of sight. Swiping right at the start of the strip draws it out,
// and letting go once it is all the way out reloads.
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

const LABEL_PULL = '引っ張って再読み込み'
const LABEL_READY = '離すと再読み込み'
const LABEL_BUSY = '読み込み中'

export class PullToReload {
  readonly zone: HTMLElement
  private readonly icon: HTMLElement
  private readonly label: HTMLElement
  /**
   * Set while the strip is still at the first card, so a swipe from there may
   * pull. The zone's `data-armed` says the same, for the tests to wait on.
   */
  private restingNow = true
  private pulling = false
  /** Set while the zone is all the way out, so letting go now reloads. */
  private ready = false
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
    this.label = document.createElement('span')
    this.label.className = 'pull-label'
    this.label.textContent = LABEL_PULL
    this.zone.append(this.icon, this.label)
    this.resting = true
    strip.addEventListener('scroll', () => this.onScroll(), { passive: true })
    // With the zone all the way out the strip cannot move any further, so a
    // trackpad still pushing sends wheel events but no scroll events. They
    // keep the pull held; over a message's frame, where they never arrive,
    // the strip coming to rest is all there is to go on.
    strip.addEventListener(
      'wheel',
      () => {
        if (this.ready) this.waitToSettle()
      },
      { passive: true },
    )
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
    // Scroll snapping would carry a pull straight back to the first card
    // (WebKit snaps before the strip reaches the start), so it is off while
    // the strip is at the first card, and comes back once the strip settles
    // further along.
    if (value) this.strip.classList.add('strip-free')
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
    this.ready = false
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
    this.ready = this.pulling && left <= 1
    this.draw(this.pulling ? 1 - left / rest : 0)
    this.waitToSettle()
  }

  private waitToSettle(): void {
    window.clearTimeout(this.settleTimer)
    this.settleTimer = window.setTimeout(() => this.settle(), SETTLE_MS)
  }

  /**
   * Once the strip stops, which is how letting go shows: reloads if the zone
   * is all the way out, puts a half-drawn zone away, or remembers that the
   * strip is at the first card.
   */
  private settle(): void {
    const rest = this.rest
    const left = this.strip.scrollLeft
    if (this.ready && left <= 1) void this.fire()
    else if (Math.abs(left - rest) <= 1) this.resting = true
    else if (left > rest) this.strip.classList.remove('strip-free')
    else if (!this.busy) this.strip.scrollTo({ left: rest, behavior: 'smooth' })
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
      if (this.strip.scrollLeft < this.rest) this.strip.scrollTo({ left: this.rest, behavior: 'smooth' })
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
