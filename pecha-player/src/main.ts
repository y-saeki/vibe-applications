// The player window: loading a deck, the run itself, and the controls.
//
// The window is always in one of four phases, kept in `data-phase` on #app so
// that the stylesheet can follow it:
//
//   empty   — nothing loaded; the loader is open
//   ready   — the cover is up, waiting for Start
//   running — slides flip every 20 seconds
//   closed  — the shutter is down, after the last slide or a Stop
//
//   empty --load--> ready --Start--> running --20 slides--> closed
//                                    running --Stop------> closed
//   closed --もう一度--> ready;   ready/closed --load--> ready

import {
  isFullscreen,
  isRemoteError,
  loadGoogleSlides,
  onGoogleSignIn,
  onResized,
  openGoogleSignIn,
  setFullscreen,
  type RemoteError,
} from './backend'
import { Deck, DeckOpenError, type OpenError } from './deck'
import { SLIDE_MS, TALK_SLIDES, formatClock, momentAt, nextChangeAt, playableSlides } from './timeline'

type Phase = 'empty' | 'ready' | 'running' | 'closed'

const $ = <T extends HTMLElement>(id: string): T => {
  const element = document.getElementById(id)
  if (!element) throw new Error(`#${id} is missing`)
  return element as T
}

const app = $('app')
const stage = $('stage')
const canvases = [$<HTMLCanvasElement>('canvas-a'), $<HTMLCanvasElement>('canvas-b')] as const
const clock = $('clock')
const counter = $('counter')
const progressFill = $('progress-fill')
const endTitle = $('end-title')
const endDetail = $('end-detail')
const dropZone = $('drop-zone')
const fileInput = $<HTMLInputElement>('file-input')
const urlForm = $<HTMLFormElement>('url-form')
const urlInput = $<HTMLInputElement>('url')
const status = $('status')
const statusText = $('status-text')
const errorText = $('error')
const signIn = $('sign-in')
const signInText = $('sign-in-text')
const signInButton = $<HTMLButtonElement>('sign-in-button')
const deckInfo = $('deck-info')
const primary = $<HTMLButtonElement>('primary')
const loadButton = $<HTMLButtonElement>('load-button')
const fullscreenButton = $<HTMLButtonElement>('fullscreen')

const isMac = navigator.userAgent.includes('Mac')

// ---------- State ----------

let phase: Phase = 'empty'
let deck: Deck | null = null
let slides = 0
let startedAt = 0
let timer: number | undefined
/** The slide on screen during a run (1-based), 0 for the cover. */
let shownSlide = 0
/** URL of the Google deck waiting on a sign-in. */
let pendingUrl: string | null = null
/** Bumped by every load, so that a slow one finishing late is ignored. */
let loadGeneration = 0

function setPhase(next: Phase): void {
  phase = next
  app.dataset.phase = next
  primary.disabled = next === 'empty'
  primary.textContent = { empty: 'スタート', ready: 'スタート', running: 'ストップ', closed: 'もう一度' }[next]
  loadButton.disabled = next === 'running'
  if (next !== 'closed') delete app.dataset.end
  if (next === 'empty') openLoader()
}

// ---------- Drawing pages ----------

/** Which canvas is on screen, and what each one holds. */
let front = 0
const held: { page: number; key: string }[] = [
  { page: 0, key: '' },
  { page: 0, key: '' },
]
let drawGeneration = 0

function sizeKey(): string {
  return `${stage.clientWidth}x${stage.clientHeight}@${window.devicePixelRatio}`
}

async function draw(index: number, page: number): Promise<boolean> {
  if (!deck) return false
  const key = sizeKey()
  if (held[index]?.page === page && held[index]?.key === key) return true
  const canvas = canvases[index as 0 | 1]
  try {
    await deck.render(page, canvas, stage.clientWidth, stage.clientHeight, window.devicePixelRatio)
  } catch {
    // Cancelled by a newer draw into the same canvas.
    return false
  }
  held[index] = { page, key }
  return true
}

/** Puts `page` on screen, then prepares the page after it behind it. */
async function showPage(page: number): Promise<void> {
  const generation = ++drawGeneration
  const back = 1 - front
  if (!(await draw(back, page)) || generation !== drawGeneration) return
  canvases[front as 0 | 1].classList.remove('front')
  canvases[back as 0 | 1].classList.add('front')
  front = back
  app.dataset.page = String(page)
  if (deck && page < deck.pageCount && phase !== 'closed') {
    void draw(1 - front, page + 1)
  }
}

let resizeFrame = 0
new ResizeObserver(() => {
  cancelAnimationFrame(resizeFrame)
  resizeFrame = requestAnimationFrame(() => {
    const page = held[front]?.page ?? 0
    if (deck && page > 0) {
      held[0] = { page: 0, key: '' }
      held[1] = { page: 0, key: '' }
      void showPage(page)
    }
  })
}).observe(stage)

// ---------- The run ----------

function updateHud(slide: number, elapsedMs: number): void {
  clock.textContent = formatClock(elapsedMs)
  counter.textContent = `${slide} / ${slides}`
}

/** Restarts the progress bar for a slide that has been up `slideElapsedMs`. */
function restartProgress(slideElapsedMs: number): void {
  progressFill.style.transition = 'none'
  progressFill.style.transform = `scaleX(${slideElapsedMs / SLIDE_MS})`
  // Commit the starting point before animating away from it.
  void progressFill.getBoundingClientRect()
  progressFill.style.transition = `transform ${SLIDE_MS - slideElapsedMs}ms linear`
  progressFill.style.transform = 'scaleX(1)'
}

function tick(): void {
  const now = performance.now()
  const moment = momentAt(startedAt, now, slides)
  if (moment.finished) {
    close('finished', moment.slide, moment.elapsedMs)
    return
  }
  if (moment.slide !== shownSlide) {
    shownSlide = moment.slide
    void showPage(moment.slide + 1)
    restartProgress(moment.slideElapsedMs)
  }
  updateHud(moment.slide, moment.elapsedMs)
  timer = window.setTimeout(tick, Math.max(0, nextChangeAt(startedAt, now, slides) - now))
}

function start(): void {
  if (phase !== 'ready' || slides === 0) return
  closeLoader()
  setPhase('running')
  startedAt = performance.now()
  shownSlide = 0
  tick()
}

function stop(): void {
  if (phase !== 'running') return
  const moment = momentAt(startedAt, performance.now(), slides)
  close('stopped', moment.slide, moment.elapsedMs)
}

function close(reason: 'finished' | 'stopped', slide: number, elapsedMs: number): void {
  window.clearTimeout(timer)
  timer = undefined
  app.dataset.end = reason
  endTitle.textContent = reason === 'finished' ? 'おつかれさまでした' : 'ストップしました'
  endDetail.textContent = `${slide} / ${slides} 枚 · ${formatClock(elapsedMs)}`
  updateHud(slide, elapsedMs)
  setPhase('closed')
}

/** Back to the cover, ready for another run. */
function rewind(): void {
  window.clearTimeout(timer)
  timer = undefined
  shownSlide = 0
  updateHud(0, 0)
  setPhase('ready')
  void showPage(1)
}

function primaryAction(): void {
  if (phase === 'ready') start()
  else if (phase === 'running') stop()
  else if (phase === 'closed') rewind()
}

// ---------- Loading ----------

function openLoader(): void {
  app.dataset.loader = ''
}

function closeLoader(): void {
  if (phase === 'empty') return
  delete app.dataset.loader
  clearMessages()
}

function clearMessages(): void {
  status.hidden = true
  errorText.hidden = true
  signIn.hidden = true
}

function showBusy(text: string): void {
  clearMessages()
  statusText.textContent = text
  status.hidden = false
}

function showError(text: string): void {
  clearMessages()
  errorText.textContent = text
  errorText.hidden = false
}

const OPEN_ERRORS: Record<OpenError, string> = {
  notPdf: 'PDF ファイルではありません。',
  password: 'パスワードのかかった PDF は開けません。',
  broken: 'PDF を開けませんでした。ファイルが壊れている可能性があります。',
  tooLarge: 'ファイルが大きすぎます（300 MB まで）。',
}

function remoteErrorText(error: RemoteError): string {
  switch (error.kind) {
    case 'invalidUrl':
      return 'Google スライドの URL ではありません。ブラウザのアドレスバーか、共有ダイアログの URL を貼り付けてください。'
    case 'signInRequired':
      return 'このスライドを開くには Google へのログインが必要です。'
    case 'forbidden':
      return 'このアカウントには、スライドを見る権限がありません。'
    case 'notFound':
      return 'スライドが見つかりません。URL を確かめてください。'
    case 'tooLarge':
      return OPEN_ERRORS.tooLarge
    case 'notPdf':
      return 'Google から PDF を受け取れませんでした。'
    case 'network':
      return `Google に接続できませんでした（${error.detail}）。`
  }
}

/** Opens `bytes` as the new deck and shows its cover. */
async function useDeck(bytes: Uint8Array, label: string, generation: number): Promise<void> {
  const next = await Deck.open(bytes)
  if (generation !== loadGeneration) {
    void next.destroy()
    return
  }
  if (playableSlides(next.pageCount) === 0) {
    void next.destroy()
    showError('表紙のほかに 1 ページ以上必要です。')
    return
  }
  const previous = deck
  deck = next
  slides = playableSlides(next.pageCount)
  held[0] = { page: 0, key: '' }
  held[1] = { page: 0, key: '' }
  void previous?.destroy()

  deckInfo.textContent =
    slides < TALK_SLIDES ? `${label} · 表紙 + ${slides} 枚（20 枚に足りません）` : `${label} · 表紙 + ${slides} 枚`
  deckInfo.classList.toggle('warning', slides < TALK_SLIDES)
  deckInfo.title = label
  pendingUrl = null
  clearMessages()
  rewind()
  delete app.dataset.loader
  primary.focus()
}

async function loadFile(file: File): Promise<void> {
  if (phase === 'running') return
  const generation = ++loadGeneration
  showBusy('読み込み中…')
  try {
    if (file.size > 300 * 1024 * 1024) throw new DeckOpenError('tooLarge')
    const bytes = new Uint8Array(await file.arrayBuffer())
    await useDeck(bytes, file.name, generation)
  } catch (err) {
    if (generation !== loadGeneration) return
    showError(err instanceof DeckOpenError ? OPEN_ERRORS[err.kind] : OPEN_ERRORS.broken)
  }
}

async function loadUrl(url: string): Promise<void> {
  if (phase === 'running') return
  const generation = ++loadGeneration
  showBusy('Google スライドから読み込み中…')
  try {
    const bytes = await loadGoogleSlides(url)
    if (generation !== loadGeneration) return
    await useDeck(bytes, 'Google スライド', generation)
  } catch (err) {
    if (generation !== loadGeneration) return
    if (isRemoteError(err) && err.kind === 'signInRequired') {
      pendingUrl = url
      clearMessages()
      signInText.textContent = remoteErrorText(err)
      signInButton.disabled = false
      signIn.hidden = false
      return
    }
    if (isRemoteError(err)) showError(remoteErrorText(err))
    else if (err instanceof DeckOpenError) showError(OPEN_ERRORS[err.kind])
    else showError(`読み込めませんでした（${String(err)}）。`)
  }
}

// ---------- Wiring ----------

primary.addEventListener('click', primaryAction)
loadButton.addEventListener('click', () => {
  if (phase !== 'running') openLoader()
})
$('close-loader').addEventListener('click', closeLoader)
$('choose-file').addEventListener('click', () => fileInput.click())
fileInput.addEventListener('change', () => {
  const file = fileInput.files?.[0]
  fileInput.value = ''
  if (file) void loadFile(file)
})
urlForm.addEventListener('submit', (event) => {
  event.preventDefault()
  const url = urlInput.value.trim()
  if (url) void loadUrl(url)
})

signInButton.addEventListener('click', () => {
  if (!pendingUrl) return
  signInButton.disabled = true
  signInText.textContent = '開いたウィンドウで Google にログインしてください。終わると自動で読み込みます。'
  openGoogleSignIn(pendingUrl).catch((err: unknown) => {
    signInButton.disabled = false
    showError(isRemoteError(err) ? remoteErrorText(err) : String(err))
  })
})
onGoogleSignIn((result) => {
  if (!pendingUrl) return
  if (result === 'done') {
    void loadUrl(pendingUrl)
  } else {
    signInText.textContent = 'ログインが終わる前にウィンドウが閉じられました。'
    signInButton.disabled = false
  }
})

// Dropping a PDF anywhere loads it, unless a run is under way.
window.addEventListener('dragover', (event) => {
  event.preventDefault()
  if (event.dataTransfer) event.dataTransfer.dropEffect = phase === 'running' ? 'none' : 'copy'
  if (phase !== 'running') {
    openLoader()
    dropZone.classList.add('dragging')
  }
})
window.addEventListener('dragleave', (event) => {
  if (event.relatedTarget === null) dropZone.classList.remove('dragging')
})
window.addEventListener('drop', (event) => {
  event.preventDefault()
  dropZone.classList.remove('dragging')
  const file = event.dataTransfer?.files[0]
  if (file && phase !== 'running') void loadFile(file)
})

// ---------- Display options ----------

type OptionName = 'clock' | 'progress' | 'counter'
const OPTIONS_KEY = 'pecha-player.options'

function readOptions(): Record<OptionName, boolean> {
  const defaults = { clock: true, progress: true, counter: true }
  try {
    const stored = JSON.parse(localStorage.getItem(OPTIONS_KEY) ?? '{}') as Partial<Record<OptionName, unknown>>
    return {
      clock: typeof stored.clock === 'boolean' ? stored.clock : defaults.clock,
      progress: typeof stored.progress === 'boolean' ? stored.progress : defaults.progress,
      counter: typeof stored.counter === 'boolean' ? stored.counter : defaults.counter,
    }
  } catch {
    return defaults
  }
}

const options = readOptions()

function applyOption(name: OptionName): void {
  const on = options[name]
  if (on) app.dataset[name] = ''
  else delete app.dataset[name]
  $(`toggle-${name}`).setAttribute('aria-pressed', String(on))
}

for (const name of ['clock', 'progress', 'counter'] as const) {
  applyOption(name)
  $(`toggle-${name}`).addEventListener('click', () => {
    options[name] = !options[name]
    applyOption(name)
    try {
      localStorage.setItem(OPTIONS_KEY, JSON.stringify(options))
    } catch {
      // Not remembered across launches, but still applied.
    }
  })
}

// ---------- Fullscreen ----------

let idleTimer: number | undefined

function markActive(): void {
  delete app.dataset.idle
  window.clearTimeout(idleTimer)
  if (app.dataset.fullscreen !== undefined) {
    idleTimer = window.setTimeout(() => {
      if (app.dataset.loader === undefined) app.dataset.idle = ''
    }, 2500)
  }
}

async function syncFullscreen(): Promise<void> {
  try {
    const on = await isFullscreen()
    if (on) app.dataset.fullscreen = ''
    else delete app.dataset.fullscreen
    fullscreenButton.title = on ? 'フルスクリーンを終了' : 'フルスクリーン'
    fullscreenButton.setAttribute('aria-label', fullscreenButton.title)
    markActive()
  } catch {
    // Outside Tauri there is no window to ask.
  }
}

async function toggleFullscreen(): Promise<void> {
  await setFullscreen(app.dataset.fullscreen === undefined)
  await syncFullscreen()
}

fullscreenButton.addEventListener('click', () => void toggleFullscreen())
onResized(() => void syncFullscreen())
window.addEventListener('mousemove', markActive)
void syncFullscreen()

// ---------- Keyboard ----------

window.addEventListener('keydown', (event) => {
  const mod = isMac ? event.metaKey : event.ctrlKey
  const typing = event.target instanceof HTMLInputElement

  if (mod && event.key.toLowerCase() === 'o') {
    event.preventDefault()
    if (phase !== 'running') openLoader()
    return
  }
  if (
    event.key === 'F11' ||
    (isMac && event.metaKey && event.ctrlKey && event.key.toLowerCase() === 'f') ||
    (!typing && !mod && !event.altKey && event.key.toLowerCase() === 'f')
  ) {
    event.preventDefault()
    void toggleFullscreen()
    return
  }
  if (event.key === 'Escape') {
    if (phase === 'running') stop()
    else if (app.dataset.loader !== undefined && phase !== 'empty') closeLoader()
    else if (app.dataset.fullscreen !== undefined) void toggleFullscreen()
    return
  }
  if (typing || mod || event.altKey) return
  // Space and Return start a run and bring back the cover afterwards. They do
  // not stop one: a reflexive press to "advance" must not end the rehearsal.
  if (event.key === ' ' || event.key === 'Enter') {
    if (phase === 'ready' || phase === 'closed') {
      event.preventDefault()
      primaryAction()
    } else if (phase === 'running') {
      event.preventDefault()
    }
  }
})

setPhase('empty')
updateHud(0, 0)
