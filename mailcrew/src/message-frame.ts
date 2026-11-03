// Showing a message's body inside a card.
//
// HTML goes into an iframe, and this is the second of two layers (the first
// is src-tauri/src/sanitize.rs, which strips anything that could run before the
// page ever sees it):
//
// - `sandbox="allow-same-origin"` and nothing else. Without `allow-scripts`
//   nothing in the message can run, whatever got past the sanitiser; without
//   `allow-forms`, `allow-popups` or `allow-top-navigation` it cannot submit,
//   open a window or move the app away. `allow-same-origin` is what lets this
//   page reach into the frame to catch link clicks and fit the width, and is
//   safe only because scripts stay off.
// - A Content-Security-Policy of its own that refuses every load but inline
//   styles and `data:` images. Images from the network are added only when the
//   user asks, card by card, so opening a message tells no sender it was read.
// - `no-referrer`, so even an allowed image does not say where it was seen.
//
// The frame never scrolls itself. It is made as tall as the message and the
// card body around it does the scrolling, so there is one scrollbar to draw
// (src/overlay-scrollbar.ts) and a sideways swipe over a card always reaches
// the strip: the document hides its overflow, and a message laid out wider
// than the card is scaled down to fit rather than left to scroll.
//
// Links leave through the Rust side, which opens http, https and mailto links
// in the default browser and refuses the rest. In WebView2 a click is caught
// here and handed to `open_link`. WebKit runs none of this page's listeners
// inside a frame that may not run scripts, so in WKWebView the click goes
// ahead as a navigation of the frame, and src-tauri/src/links.rs turns that
// navigation away and opens the link outside instead. WebKit checks the app's
// `frame-src` before it asks, so src-tauri/tauri.macos.conf.json lets web and
// mail links through it on macOS; otherwise the click would do nothing.

import type { CardBody } from './backend'

const BASE_STYLE = `
html { color-scheme: light; background: #fff; overflow: hidden; }
body {
  margin: 16px;
  color: #1d1d1f;
  font: 14px/1.6 -apple-system, BlinkMacSystemFont, "Hiragino Sans", "Yu Gothic UI", "Segoe UI", sans-serif;
  overflow-wrap: anywhere;
}
pre { white-space: pre-wrap; }
a { color: #1f5fbf; }
`

/** The Content-Security-Policy a message's frame runs under. */
export function framePolicy(allowRemoteImages: boolean): string {
  const images = allowRemoteImages ? "data: https: http:" : 'data:'
  return `default-src 'none'; style-src 'unsafe-inline'; img-src ${images}`
}

function escapeAttribute(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;')
}

/** The whole document the frame is given, around the sanitised body. */
export function frameDocument(html: string, allowRemoteImages: boolean): string {
  return [
    '<!doctype html><html><head><meta charset="utf-8">',
    `<meta http-equiv="Content-Security-Policy" content="${escapeAttribute(framePolicy(allowRemoteImages))}">`,
    '<meta name="referrer" content="no-referrer">',
    `<style>${BASE_STYLE}</style>`,
    '</head><body>',
    html,
    '</body></html>',
  ].join('')
}

/** Hands a link to the Rust side; anything it will not open is dropped there. */
type OpenLink = (url: string) => void

/** Follows clicks on links inside `root` without ever navigating. */
function catchLinks(root: Document | HTMLElement, openLink: OpenLink): void {
  const follow = (event: MouseEvent) => {
    const target = event.target as Element | null
    const anchor = target?.closest?.('a[href]')
    if (!anchor) return
    event.preventDefault()
    if (event.type === 'auxclick' && event.button !== 1) return
    const href = anchor.getAttribute('href') ?? ''
    if (href.startsWith('#')) return
    openLink(href)
  }
  root.addEventListener('click', follow as EventListener, true)
  root.addEventListener('auxclick', follow as EventListener, true)
}

/**
 * How tall the document is, in the frame's pixels with the zoom applied. The
 * two readings agree in Chromium; the larger is taken so that an engine that
 * reports one of them before the zoom leaves room to spare rather than cutting
 * the end of the message off.
 */
function contentHeight(root: HTMLElement): number {
  return Math.ceil(Math.max(root.getBoundingClientRect().height, root.scrollHeight))
}

/**
 * Scales a message down when it is laid out wider than the card, as
 * newsletters built for a 600-pixel column are, and makes the frame as tall as
 * what it then holds. Narrower messages are left at their own size.
 *
 * The frame is shrunk to nothing for a moment to read the height, since a
 * document is never shorter than the frame showing it; the card body's scroll
 * position is put back afterwards, before anything is painted.
 */
function fit(frame: HTMLIFrameElement): void {
  const root = frame.contentDocument?.documentElement
  if (!root) return
  const scroller = frame.parentElement
  const scrollTop = scroller?.scrollTop ?? 0
  root.style.removeProperty('zoom')
  const available = frame.clientWidth
  const wanted = root.scrollWidth
  if (available > 0 && wanted > available + 1) root.style.setProperty('zoom', String(available / wanted))
  frame.style.height = '0px'
  frame.style.height = `${contentHeight(root)}px`
  if (scroller) scroller.scrollTop = scrollTop
}

export interface MountedBody {
  /** Reloads the frame with images from the network let through. */
  allowRemoteImages(): void
  /** Lays the message out again for the card's current width. */
  refit(): void
}

const URL_PATTERN = /\b(https?:\/\/[^\s<>"'）」】]+)/g

/** Plain text, with its web addresses made into links. */
function renderText(container: HTMLElement, text: string, openLink: OpenLink): void {
  const body = document.createElement('div')
  body.className = 'message-text'
  let last = 0
  for (const match of text.matchAll(URL_PATTERN)) {
    const url = match[0].replace(/[.,;:!?)]+$/, '')
    const at = match.index ?? 0
    body.append(text.slice(last, at))
    const link = document.createElement('a')
    link.href = url
    link.textContent = url
    body.append(link)
    last = at + url.length
  }
  body.append(text.slice(last))
  catchLinks(body, openLink)
  container.append(body)
}

/** Puts a message's body into `container`, which it fills. */
export function mountBody(container: HTMLElement, body: CardBody, openLink: OpenLink): MountedBody {
  container.replaceChildren()
  if (body.kind === 'text') {
    renderText(container, body.text, openLink)
    return { allowRemoteImages() {}, refit() {} }
  }
  if (body.kind === 'empty') {
    const empty = document.createElement('p')
    empty.className = 'message-empty'
    empty.textContent = '本文はありません'
    container.append(empty)
    return { allowRemoteImages() {}, refit() {} }
  }
  const frame = document.createElement('iframe')
  frame.className = 'message-frame'
  frame.setAttribute('sandbox', 'allow-same-origin')
  frame.setAttribute('referrerpolicy', 'no-referrer')
  frame.setAttribute('title', 'メール本文')
  frame.addEventListener('load', () => {
    const doc = frame.contentDocument
    if (!doc) return
    catchLinks(doc, openLink)
    fit(frame)
  })
  frame.srcdoc = frameDocument(body.html, false)
  container.append(frame)
  return {
    allowRemoteImages() {
      frame.srcdoc = frameDocument(body.html, true)
    },
    refit() {
      fit(frame)
    },
  }
}
