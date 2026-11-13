// Opening a PDF and drawing its pages, through PDF.js.
//
// A deck may come from anywhere — a file someone sent, a link — so PDF.js
// runs with everything beyond drawing switched off: no scripting (only the
// viewer component can run document JavaScript, and it is not used), no XFA
// forms, no annotation layer (so no clickable links), and only the pages'
// pixels reach the DOM, as a canvas. Parsing happens in PDF.js's worker.

import {
  GlobalWorkerOptions,
  PasswordException,
  getDocument,
  type PDFDocumentLoadingTask,
  type PDFDocumentProxy,
  type RenderTask,
} from 'pdfjs-dist/legacy/build/pdf.mjs'

/** Largest file we open, matching `MAX_PDF_BYTES` on the Rust side. */
export const MAX_PDF_BYTES = 300 * 1024 * 1024

/** Where build.mjs puts PDF.js's worker and data files. */
const ASSETS = new URL('pdfjs/', document.baseURI).href
GlobalWorkerOptions.workerSrc = `${ASSETS}pdf.worker.min.mjs`

export type OpenError = 'notPdf' | 'password' | 'broken' | 'tooLarge'

export class DeckOpenError extends Error {
  constructor(readonly kind: OpenError) {
    super(kind)
  }
}

/** PDF files start with `%PDF-`, which the spec allows within the first kilobyte. */
export function looksLikePdf(bytes: Uint8Array): boolean {
  const head = bytes.subarray(0, 1024)
  for (let i = 0; i + 5 <= head.length; i++) {
    if (
      head[i] === 0x25 &&
      head[i + 1] === 0x50 &&
      head[i + 2] === 0x44 &&
      head[i + 3] === 0x46 &&
      head[i + 4] === 0x2d
    ) {
      return true
    }
  }
  return false
}

export class Deck {
  private tasks = new Map<HTMLCanvasElement, RenderTask>()
  private latest = new Map<HTMLCanvasElement, number>()
  private sequence = 0

  private constructor(
    private readonly task: PDFDocumentLoadingTask,
    private readonly pdf: PDFDocumentProxy,
  ) {}

  static async open(bytes: Uint8Array): Promise<Deck> {
    if (bytes.byteLength > MAX_PDF_BYTES) throw new DeckOpenError('tooLarge')
    if (!looksLikePdf(bytes)) throw new DeckOpenError('notPdf')
    const task = getDocument({
      data: bytes,
      enableXfa: false,
      // Only our own files: the CMaps and standard fonts that decks without
      // embedded Japanese fonts need, and the image decoders' wasm.
      cMapUrl: `${ASSETS}cmaps/`,
      standardFontDataUrl: `${ASSETS}standard_fonts/`,
      wasmUrl: `${ASSETS}wasm/`,
      iccUrl: `${ASSETS}iccs/`,
      // Pages are drawn on this thread, so the separate renderer worker is
      // not shipped.
      disableWorkerRendering: true,
    })
    try {
      return new Deck(task, await task.promise)
    } catch (err) {
      void task.destroy()
      if (err instanceof PasswordException) throw new DeckOpenError('password')
      throw new DeckOpenError('broken')
    }
  }

  get pageCount(): number {
    return this.pdf.numPages
  }

  /**
   * Draws page `n` (1-based) into `canvas`, as large as fits in a box of
   * `width` × `height` CSS pixels, sharp at `pixelRatio`. A newer draw into the
   * same canvas supersedes an unfinished one, which then rejects.
   */
  async render(n: number, canvas: HTMLCanvasElement, width: number, height: number, pixelRatio: number): Promise<void> {
    // The newest call for a canvas is the one that counts. Checked after every
    // await, since an older call still waiting for its page would otherwise
    // carry on and overwrite (or cancel) the newer one.
    const token = ++this.sequence
    this.latest.set(canvas, token)
    const superseded = () => this.latest.get(canvas) !== token
    this.tasks.get(canvas)?.cancel()
    const page = await this.pdf.getPage(n)
    if (superseded()) throw new Error('superseded')
    const base = page.getViewport({ scale: 1 })
    const fit = Math.min(width / base.width, height / base.height)
    const viewport = page.getViewport({ scale: fit * pixelRatio })
    // Drawn off to the side first, so a canvas being reused never shows a
    // half-drawn page.
    const scratch = document.createElement('canvas')
    scratch.width = Math.max(1, Math.floor(viewport.width))
    scratch.height = Math.max(1, Math.floor(viewport.height))
    const task = page.render({ canvas: scratch, viewport })
    this.tasks.set(canvas, task)
    try {
      await task.promise
    } finally {
      if (this.tasks.get(canvas) === task) this.tasks.delete(canvas)
    }
    if (superseded()) throw new Error('superseded')
    canvas.width = scratch.width
    canvas.height = scratch.height
    canvas.style.width = `${scratch.width / pixelRatio}px`
    canvas.style.height = `${scratch.height / pixelRatio}px`
    canvas.getContext('2d')?.drawImage(scratch, 0, 0)
  }

  async destroy(): Promise<void> {
    for (const task of this.tasks.values()) task.cancel()
    this.tasks.clear()
    await this.task.destroy()
  }
}
