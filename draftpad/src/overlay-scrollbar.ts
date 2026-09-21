// draftpad's own scrollbar, laid over what scrolls rather than beside it.
//
// A platform scrollbar is a column of the layout. The moment the content
// outgrows its box the bar appears and takes its width out of it, so dragging
// the window taller and shorter makes the draft narrower and wider along with
// it, and every wrapped line reflows on the way. An overlay bar sits on top of
// the content and leaves its width alone.
//
// No engine offers one as a setting. `overflow: overlay` has been an alias of
// `overflow: auto` in Chromium since 114, giving `::-webkit-scrollbar` a width
// turns that bar from an overlay into a classic one, and macOS draws overlay
// bars only while the system setting says to. So style.css hides the platform's
// bars on the boxes draftpad scrolls, and this draws the thumb for them.
//
// Only the vertical bar is drawn. Hiding the platform's cannot be done on one
// axis alone, so there is no horizontal one either; what keeps that from
// putting anything out of reach is that neither box overflows sideways, which
// the E2E suite holds them to.
//
// The geometry stays in the stylesheet. The bar is handed two numbers — how
// much of the content the view covers, and how far through it the view sits —
// and style.css turns them into a length, the way it does for the numbers
// src/indent-guides.ts hands over.

/** How long the bar stays up after the scrolling stops. */
const IDLE_MS = 800

/** Where a drag of the thumb started, and what one pixel of it is worth. */
interface Drag {
  /** Where the pointer went down. */
  from: number
  /** How far through the content the view sat then. */
  offset: number
  /** How far the thumb may travel, and how far the content may. */
  travel: number
  range: number
}

function clamp01(value: number): number {
  return value < 0 ? 0 : value > 1 ? 1 : value
}

/** The strip, the thumb inside it, and everything that places and shows them. */
class OverlayScrollbar {
  private readonly track = document.createElement('div')
  private readonly thumb = document.createElement('div')
  private drag: Drag | null = null
  private idle: number | undefined
  private hovered = false

  /**
   * @param scroller the box whose overflow this bar stands for
   * @param host where the strip is appended. It is positioned against the
   *   viewport, so this only decides what it is painted over: the preferences
   *   panel's bar has to go inside the dialog, which is in the top layer.
   * @param content an element inside the scroller whose size follows the
   *   content, for a scroller that does not change size as what it holds does
   */
  constructor(
    private readonly scroller: HTMLElement,
    host: HTMLElement,
    content?: HTMLElement,
  ) {
    this.track.className = 'scrollbar'
    // The scroll container itself is what a screen reader and the keyboard
    // already reach; this is the platform's bar drawn again, not a second
    // control, and announcing it as one would only be in the way.
    this.track.setAttribute('aria-hidden', 'true')
    this.thumb.className = 'scrollbar-thumb'
    // A bar stands for overflow, and there is none to stand for until the
    // first measurement says otherwise.
    this.track.hidden = true
    this.track.append(this.thumb)
    host.append(this.track)

    scroller.addEventListener('scroll', () => this.scrolled(), { passive: true })
    // A scroll is the only thing that puts the bar up. A pointer resting over
    // the box does not: it rests there the whole time someone is writing, and
    // a bar that never goes away is the chrome this one is drawn to avoid.
    // Once it is up, the hand reaching for the thumb keeps it up.
    this.thumb.addEventListener('pointerenter', () => this.setHovered(true))
    this.thumb.addEventListener('pointerleave', () => this.setHovered(false))
    this.thumb.addEventListener('pointerdown', (event) => this.startDrag(event))
    this.thumb.addEventListener('pointermove', (event) => this.moveDrag(event))
    this.thumb.addEventListener('pointerup', (event) => this.endDrag(event))
    this.thumb.addEventListener('pointercancel', (event) => this.endDrag(event))

    // The strip follows the scrollport, and the thumb follows the content. The
    // observer measures once on its own as soon as it starts, which is what
    // places the bar to begin with.
    const observer = new ResizeObserver(() => this.measure())
    observer.observe(scroller)
    if (content) observer.observe(content)
    // Resizing the window moves the preferences panel without resizing it,
    // and nothing above notices that.
    window.addEventListener('resize', () => this.measure())
  }

  /**
   * Lays the strip along the scrollport's right edge and re-reads the geometry.
   *
   * The strip is positioned against the viewport rather than against the host,
   * so neither element has to be a containing block for it, and the scroller
   * may move inside the host — which it does when the search panel opens above
   * the draft — without anything to keep in step.
   */
  private measure(): void {
    const rect = this.scroller.getBoundingClientRect()
    // The padding box is what scrolls: .panel has a border, .cm-scroller does
    // not, and the strip belongs inside it either way.
    const left = rect.left + this.scroller.clientLeft
    const { clientWidth, clientHeight } = this.scroller
    this.track.style.top = `${rect.top + this.scroller.clientTop}px`
    this.track.style.height = `${clientHeight}px`
    this.track.style.right = `${window.innerWidth - (left + clientWidth)}px`
    this.sync()
  }

  /** Re-reads how far through the content the view sits. */
  private sync(): void {
    const { clientHeight: view, scrollHeight: content, scrollTop: offset } = this.scroller
    const range = content - view
    this.track.hidden = range <= 0
    if (this.track.hidden) return
    // Rubber-band overscroll takes the offset past either end, which would
    // carry the thumb out of the strip with it.
    this.set('--scrollbar-cover', view / content)
    this.set('--scrollbar-progress', offset / range)
  }

  private set(name: string, value: number): void {
    this.track.style.setProperty(name, clamp01(value).toFixed(4))
  }

  private scrolled(): void {
    this.sync()
    this.wake()
  }

  private setHovered(hovered: boolean): void {
    this.hovered = hovered
    this.wake()
  }

  /** Brings the bar up, and fades it out once the scrolling has stopped. */
  private wake(): void {
    this.track.toggleAttribute('data-shown', true)
    if (this.idle !== undefined) window.clearTimeout(this.idle)
    this.idle = window.setTimeout(() => {
      this.idle = undefined
      // Nothing is re-armed here: whatever is holding the bar up lets go
      // through a pointerleave, and that comes back through setHovered.
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
    // Otherwise the press starts a selection in the draft underneath and takes
    // the keyboard off the editor along with it.
    event.preventDefault()
    this.thumb.setPointerCapture(event.pointerId)
    this.track.toggleAttribute('data-dragging', true)
  }

  private moveDrag(event: PointerEvent): void {
    const drag = this.drag
    // A strip with no room to travel holds a thumb that fills it, which only
    // happens at the minimum length; there is nowhere to drag it to.
    if (!drag || drag.travel <= 0) return
    this.scroller.scrollTop = drag.offset + ((event.clientY - drag.from) / drag.travel) * drag.range
  }

  private endDrag(event: PointerEvent): void {
    if (!this.drag) return
    this.drag = null
    this.thumb.releasePointerCapture(event.pointerId)
    this.track.toggleAttribute('data-dragging', false)
  }
}

/**
 * Gives `scroller` an overlay scrollbar.
 *
 * @param scroller the box that scrolls
 * @param host where the bar is appended, which decides what it is painted
 *   over; it positions itself against the viewport either way
 * @param content an element inside the scroller whose size follows the content,
 *   for a scroller that does not change size as what it holds does
 */
export function overlayScrollbar(scroller: HTMLElement, host: HTMLElement, content?: HTMLElement): void {
  new OverlayScrollbar(scroller, host, content)
}
