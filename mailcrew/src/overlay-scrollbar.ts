// The vertical scrollbar of a card's body, drawn over the message rather than
// beside it, and only while it is being scrolled.
//
// A strip of cards each showing the platform's scrollbar is a row of grey bars
// that are of no use until someone scrolls. macOS only hides them while the
// system setting says to, and WebView2 never does, so style.css hides the
// platform's bar on the card body and this draws a thin thumb in its place.
// Hiding the platform's bar takes the horizontal one with it, which is fine:
// a card body never overflows sideways (src/message-frame.ts).
//
// The thumb's geometry stays in the stylesheet: it is handed how much of the
// content the view covers and how far through it the view sits.

/** How long the bar stays up after the scrolling stops. */
const IDLE_MS = 800

interface Drag {
  from: number
  offset: number
  travel: number
  range: number
}

function clamp01(value: number): number {
  return value < 0 ? 0 : value > 1 ? 1 : value
}

export class OverlayScrollbar {
  private readonly track = document.createElement('div')
  private readonly thumb = document.createElement('div')
  private readonly observer: ResizeObserver
  private drag: Drag | null = null
  private idle: number | undefined
  private hovered = false

  /**
   * @param scroller the box that scrolls; the track is laid over its right edge
   *   from inside `host`, which has to be its positioned parent
   */
  constructor(
    private readonly scroller: HTMLElement,
    host: HTMLElement,
  ) {
    this.track.className = 'scrollbar'
    this.track.setAttribute('aria-hidden', 'true')
    this.track.hidden = true
    this.thumb.className = 'scrollbar-thumb'
    this.track.append(this.thumb)
    host.append(this.track)

    scroller.addEventListener(
      'scroll',
      () => {
        this.sync()
        this.wake()
      },
      { passive: true },
    )
    this.thumb.addEventListener('pointerenter', () => this.setHovered(true))
    this.thumb.addEventListener('pointerleave', () => this.setHovered(false))
    this.thumb.addEventListener('pointerdown', (event) => this.startDrag(event))
    this.thumb.addEventListener('pointermove', (event) => this.moveDrag(event))
    this.thumb.addEventListener('pointerup', (event) => this.endDrag(event))
    this.thumb.addEventListener('pointercancel', (event) => this.endDrag(event))

    this.observer = new ResizeObserver(() => this.sync())
    this.observer.observe(scroller)
  }

  /** Follows the content as well, whose height can change without the box's. */
  watch(content: Element): void {
    this.observer.observe(content)
  }

  private sync(): void {
    const { clientHeight: view, scrollHeight: content, scrollTop: offset } = this.scroller
    const range = content - view
    this.track.hidden = range <= 0
    if (this.track.hidden) return
    this.track.style.setProperty('--scrollbar-cover', clamp01(view / content).toFixed(4))
    this.track.style.setProperty('--scrollbar-progress', clamp01(offset / range).toFixed(4))
  }

  private setHovered(hovered: boolean): void {
    this.hovered = hovered
    this.wake()
  }

  private wake(): void {
    this.track.toggleAttribute('data-shown', true)
    window.clearTimeout(this.idle)
    this.idle = window.setTimeout(() => {
      this.idle = undefined
      if (this.hovered || this.drag) return
      this.track.toggleAttribute('data-shown', false)
    }, IDLE_MS)
  }

  private startDrag(event: PointerEvent): void {
    const { clientHeight: view, scrollHeight: content, scrollTop: offset } = this.scroller
    this.drag = {
      from: event.clientY,
      offset,
      travel: view - this.thumb.getBoundingClientRect().height,
      range: content - view,
    }
    event.preventDefault()
    this.thumb.setPointerCapture(event.pointerId)
    this.track.toggleAttribute('data-dragging', true)
  }

  private moveDrag(event: PointerEvent): void {
    const drag = this.drag
    if (!drag || drag.travel <= 0) return
    this.scroller.scrollTop = drag.offset + ((event.clientY - drag.from) / drag.travel) * drag.range
  }

  private endDrag(event: PointerEvent): void {
    if (!this.drag) return
    this.drag = null
    this.thumb.releasePointerCapture(event.pointerId)
    this.track.toggleAttribute('data-dragging', false)
    this.wake()
  }
}
