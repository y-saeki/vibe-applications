import './style.css'

import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

import { comboFromEvent, createCommands, indexByKeys } from './commands'
import { Editor } from './editor'
import { defaultFontFamily } from './fonts'
import { Preferences } from './preferences'
import { clamp, FONT_SIZE_MAX, FONT_SIZE_MIN, loadState, Store, type StateKey } from './state'
import { StatusBar } from './statusbar'
import { ThemeController } from './theme'

const COUNT_DEBOUNCE_MS = 100
const RESIZE_DEBOUNCE_MS = 500

function countCodePoints(text: string): number {
  let count = 0
  for (const _ of text) count++
  return count
}

function debounce(ms: number, fn: () => void): () => void {
  let timer: number | undefined
  return () => {
    if (timer !== undefined) window.clearTimeout(timer)
    timer = window.setTimeout(() => {
      timer = undefined
      fn()
    }, ms)
  }
}

function byId<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id)
  if (!element) throw new Error(`missing element #${id}`)
  return element as T
}

async function main(): Promise<void> {
  const { state, platform, version, openPreferences: startWithPreferences } = await loadState()
  const isMac = platform === 'macos'
  document.documentElement.dataset.platform = platform

  const store = new Store(state)
  const appWindow = getCurrentWindow()
  const fontFamily = defaultFontFamily(platform)

  let editor: Editor | undefined
  const theme = new ThemeController(state.theme, (resolved) => editor?.setDark(resolved === 'dark'))

  // The preferences panel only exists further down, so the gear button goes
  // through a late-bound reference to the same action the command table gets.
  let openPreferences = (): void => {}
  const statusBar = new StatusBar(byId('statusbar'), {
    onLanguageChange: (language) => store.set({ language }),
    onAlwaysOnTopChange: (alwaysOnTop) => store.set({ alwaysOnTop }),
    onOpenPreferences: () => openPreferences(),
  })
  statusBar.setLanguage(state.language)
  statusBar.setAlwaysOnTop(state.alwaysOnTop)

  const updateCounts = debounce(COUNT_DEBOUNCE_MS, () => {
    if (editor) statusBar.setCounts(countCodePoints(editor.getText()), editor.lineCount)
  })

  editor = await Editor.create({
    parent: byId('editor'),
    initial: state,
    defaultFontFamily: fontFamily,
    dark: document.documentElement.dataset.theme === 'dark',
    onDocChanged: () => {
      store.markTextChanged()
      updateCounts()
    },
  })
  const ed = editor
  store.setTextProvider(() => ed.getText())
  statusBar.setCounts(countCodePoints(ed.getText()), ed.lineCount)

  // ---- quitting ----------------------------------------------------------
  let quitting = false
  const quit = async (): Promise<void> => {
    if (quitting) return
    quitting = true
    try {
      await store.flush()
    } catch (err) {
      console.error('draftpad: flush before quit failed', err)
    }
    await invoke('quit_app')
  }
  await appWindow.onCloseRequested(async (event) => {
    event.preventDefault()
    await quit()
  })
  window.addEventListener('blur', () => void store.flush())

  // ---- preferences --------------------------------------------------------
  const focusEditor = (): void => ed.focus()
  const preferences = new Preferences(byId('preferences'), store, { version, defaultFontFamily: fontFamily, onClose: focusEditor })

  openPreferences = () => preferences.open()

  const commands = createCommands(
    {
      openPreferences,
      // The preferences panel owns the keyboard while it is open, so the
      // editor's history stays out of the way.
      undo: () => {
        if (!preferences.isOpen) ed.undo()
      },
      redo: () => {
        if (!preferences.isOpen) ed.redo()
      },
      quit,
      toggleFullscreen: async () => appWindow.setFullscreen(!(await appWindow.isFullscreen())),
      changeFontSize: (delta) => store.set({ fontSize: clamp(store.state.fontSize + delta, FONT_SIZE_MIN, FONT_SIZE_MAX) }),
      openSearch: () => ed.openSearch(),
    },
    platform,
  )
  const commandById = new Map(commands.map((command) => [command.id, command]))

  // ---- reacting to settings ----------------------------------------------
  const apply: Partial<Record<StateKey, () => void>> = {
    language: () => {
      statusBar.setLanguage(store.state.language)
      void ed.setLanguage(store.state.language)
    },
    editorMode: () => void ed.setVim(store.state.editorMode === 'vim'),
    theme: () => theme.set(store.state.theme),
    fontSize: () => ed.setFont(store.state.fontSize, store.state.fontFamily),
    fontFamily: () => ed.setFont(store.state.fontSize, store.state.fontFamily),
    tabSize: () => ed.setTabSize(store.state.tabSize),
    quickSuggestions: () => ed.setQuickSuggestions(store.state.quickSuggestions),
    alwaysOnTop: () => {
      statusBar.setAlwaysOnTop(store.state.alwaysOnTop)
      void appWindow.setAlwaysOnTop(store.state.alwaysOnTop)
    },
  }
  store.subscribe((_, changed) => {
    for (const key of changed) apply[key]?.()
  })
  if (state.alwaysOnTop) void appWindow.setAlwaysOnTop(true)

  // ---- menu (macOS) and keyboard shortcuts (elsewhere) -------------------
  await listen<string>('menu', (event) => {
    void commandById.get(event.payload)?.run()
  })
  if (!isMac) {
    const byKeys = indexByKeys(commands)
    window.addEventListener(
      'keydown',
      (event) => {
        const combo = comboFromEvent(event, platform)
        const command = combo ? byKeys.get(combo) : undefined
        if (!command) return
        event.preventDefault()
        event.stopPropagation()
        void command.run()
      },
      true,
    )
  }
  window.addEventListener(
    'keydown',
    (event) => {
      if (event.key !== 'Escape') return
      if (preferences.isOpen) {
        event.preventDefault()
        preferences.close()
      }
    },
    true,
  )

  // ---- window size (position is intentionally not remembered) ------------
  window.addEventListener(
    'resize',
    debounce(RESIZE_DEBOUNCE_MS, () => {
      void (async () => {
        if ((await appWindow.isFullscreen()) || (await appWindow.isMaximized())) return
        const size = (await appWindow.innerSize()).toLogical(await appWindow.scaleFactor())
        const width = Math.round(size.width)
        const height = Math.round(size.height)
        if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) return
        store.set({ windowWidth: width, windowHeight: height })
      })()
    }),
  )

  // ---- the caret and the window ------------------------------------------
  // Windows anchors the IME's composition string and its candidate list to the
  // caret. A window that is activated with nothing focused inside it has no
  // caret to offer, so both end up at the top-left of the screen instead of
  // over the editor. Keeping something focused whenever the window is active
  // is what stops that.
  window.addEventListener('focus', () => {
    const focused = document.activeElement
    if (focused && focused !== document.body) return
    if (preferences.isOpen) preferences.focus()
    else ed.focus()
  })

  ed.focus()
  if (startWithPreferences) openPreferences()
  // The window has been hidden since launch for the same reason: it becomes
  // visible, and takes the keyboard, only now that the caret is in place.
  await appWindow.show()
}

main().catch((err: unknown) => {
  console.error(err)
  document.body.textContent = `draftpad の起動に失敗しました: ${String(err)}`
  void getCurrentWindow().show()
})
