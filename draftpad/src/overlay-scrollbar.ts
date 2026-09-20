// draftpad's own scrollbars, laid over what scrolls rather than beside it.
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
// bars on the boxes draftpad scrolls, and this draws the thumbs for them.
//
// The geometry stays in the stylesheet. Each bar is handed two numbers — how
// much of the content the view covers, and how far through it the view sits —
// and style.css turns them into a length, the way it does for the numbers
// src/indent-guides.ts hands over.

/** How long a bar stays up once nothing is holding it any more. */
const IDLE_MS = 800

type Axis = 'x' | 'y'

/** Where a drag of the thumb started, and what one pixel of it is worth. */
interface Drag {
  /** Where the pointer went down, along the axis. */
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

/** One bar: the strip, the thumb inside it, and the numbers that place it. */
class Bar {
  private readonly track = document.createElement('div')
  private readonly thumb = document.createElement('div')
  private drag: Drag | null = null

  /**
   * @param scroller the box whose overflow this bar stands for
   * @param host where the strip is appended. It is positioned against the
   *   viewport, so this only decides what it is painted over: the preferences
   *   panel's bar has to go inside the dialog, which is in the top layer.
   * @param axis which of the two overflows this bar carries
   * @param onHover told whether the pointer is on the thumb, so the owner can
   *   keep the bar up while someone reaches for it
   */
  constructor(
    private readonly scroller: HTMLElement,
    host: HTMLElement,
    private readonly axis: Axis,
    onHover: (hovered: boolean) => void,
  ) {
    this.track.className = `scrollbar scrollbar-${axis}`
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

    this.thumb.addEventListener('pointerenter', () => onHover(true))
    this.thumb.addEventListener('pointerleave', () => onHover(false))
    this.thumb.addEventListener('pointerdown', (event) => this.startDrag(event))
    this.thumb.addEventListener('pointermove', (event) => this.moveDrag(event))
    this.thumb.addEventListener('pointerup', (event) => this.endDrag(event))
    this.thumb.addEventListener('pointercancel', (event) => this.endDrag(event))
  }

  /** True while the thumb is being dragged. */
  get dragging(): boolean {
    return this.drag !== null
  }

  /** Whether the bar is drawn at all. */
  setShown(shown: boolean): void {
    this.track.toggleAttribute('data-shown', shown)
  }

  /**
   * Lays the strip along the scrollport's edge and re-reads the geometry.
   *
   * The strip is positioned against the viewport rather than against the host,
   * so neither element has to be a containing block for it, and the scroller
   * may move inside the host — which it does when the search panel opens above
   * the draft — without anything to keep in step.
   */
  measure(): void {
    const rect = this.scroller.getBoundingClientRect()
    // The padding box is what scrolls: .panel has a border, .cm-scroller does
    // not, and the strip belongs inside it either way.
    const left = rect.left + this.scroller.clientLeft
    const top = rect.top + this.scroller.clientTop
    const { clientWidth: width, clientHeight: height } = this.scroller
    const style = this.track.style
    if (this.axis === 'y') {
      style.top = `${top}px`
      style.height = `${height}px`
      style.right = `${window.innerWidth - (left + width)}px`
    } else {
      style.left = `${left}px`
      style.width = `${width}px`
      style.bottom = `${window.innerHeight - (top + height)}px`
    }
    this.sync()
  }

  /** Re-reads how far through the content the view sits. */
  sync(): void {
    const vertical = this.axis === 'y'
    const view = vertical ? this.scroller.clientHeight : this.scroller.clientWidth
    const content = vertical ? this.scroller.scrollHeight : this.scroller.scrollWidth
    const offset = vertical ? this.scroller.scrollTop : this.scroller.scrollLeft
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

  private along(event: PointerEvent): number {
    return this.axis === 'y' ? event.clientY : event.clientX
  }

  private startDrag(event: PointerEvent): void {
    const vertical = this.axis === 'y'
    const view = vertical ? this.scroller.clientHeight : this.scroller.clientWidth
    const content = vertical ? this.scroller.scrollHeight : this.scroller.scrollWidth
    const thumb = this.thumb.getBoundingClientRect()
    this.drag = {
      from: this.along(event),
      offset: vertical ? this.scroller.scrollTop : this.scroller.scrollLeft,
      travel: view - (vertical ? thumb.height : thumb.width),
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
    if (!drag) return
    // A strip with no room to travel is a thumb that fills it, which only
    // happens at the minimum length; there is nothing to drag it to.
    const moved = drag.travel > 0 ? ((this.along(event) - drag.from) / drag.travel) * drag.range : 0
    if (this.axis === 'y') this.scroller.scrollTop = drag.offset + moved
    else this.scroller.scrollLeft = drag.offset + moved
  }

  private endDrag(event: PointerEvent): void {
    if (!this.drag) return
    this.drag = null
    this.thumb.releasePointerCapture(event.pointerId)
    this.track.toggleAttribute('data-dragging', false)
  }
}

/** The two bars of one scroller, and everything that brings them up. */
class OverlayScrollbars {
  private readonly bars: Bar[]
  private idle: number | undefined
  private hovered = false

  constructor(scroller: HTMLElement, host: HTMLElement, content?: HTMLElement) {
    const onHover = (hovered: boolean): void => {
      this.hovered = hovered
      this.wake()
    }
    this.bars = [new Bar(scroller, host, 'y', onHover), new Bar(scroller, host, 'x', onHover)]

    scroller.addEventListener('scroll', () => this.scrolled(), { passive: true })
    scroller.addEventListener('pointerenter', () => onHover(true))
    scroller.addEventListener('pointerleave', () => onHover(false))

    // The strips follow the scrollport, and the thumbs follow the content. A
    // scroller that has been given a second element to watch — the editor,
    // whose content grows as the draft does without the scroller itself
    // changing size — reports both. The observer measures once on its own as
    // soon as it starts, which is what places the bars to begin with.
    const observer = new ResizeObserver(() => this.measure())
    observer.observe(scroller)
    if (content) observer.observe(content)
    // Resizing the window moves the preferences panel without resizing it,
    // and nothing above notices that.
    window.addEventListener('resize', () => this.measure())
  }

  private measure(): void {
    for (const bar of this.bars) bar.measure()
  }

  private scrolled(): void {
    for (const bar of this.bars) bar.sync()
    this.wake()
  }

  /** Brings the bars up, and takes them down once nothing is holding them. */
  private wake(): void {
    for (const bar of this.bars) bar.setShown(true)
    if (this.idle !== undefined) window.clearTimeout(this.idle)
    this.idle = window.setTimeout(() => {
      this.idle = undefined
      // Nothing is re-armed here: whatever is holding the bars up lets go
      // through a pointerleave, and that comes back through onHover.
      if (this.hovered || this.bars.some((bar) => bar.dragging)) return
      for (const bar of this.bars) bar.setShown(false)
    }, IDLE_MS)
  }
}

/**
 * Gives `scroller` a pair of overlay scrollbars.
 *
 * @param scroller the box that scrolls
 * @param host where the bars are appended, which decides what they are painted
 *   over; they position themselves against the viewport either way
 * @param content an element inside the scroller whose size follows the content,
 *   for a scroller that does not change size as what it holds does
 */
export function overlayScrollbars(scroller: HTMLElement, host: HTMLElement, content?: HTMLElement): void {
  new OverlayScrollbars(scroller, host, content)
}
