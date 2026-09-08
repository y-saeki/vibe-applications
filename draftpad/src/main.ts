import './style.css'

import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

import { comboFromEvent, createCommands, indexByKeys, type Command } from './commands'
import { Editor } from './editor'
import { defaultFontFamily } from './fonts'
import { Palette } from './palette'
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
  const { state, platform, version } = await loadState()
  const isMac = platform === 'macos'
  document.documentElement.dataset.platform = platform

  const store = new Store(state)
  const appWindow = getCurrentWindow()
  const fontFamily = defaultFontFamily(platform)

  let editor: Editor | undefined
  const theme = new ThemeController(state.theme, (resolved) => editor?.setDark(resolved === 'dark'))

  const statusBar = new StatusBar(byId('statusbar'), {
    onLanguageChange: (language) => store.set({ language }),
    onAlwaysOnTopChange: (alwaysOnTop) => store.set({ alwaysOnTop }),
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

  // ---- overlays -----------------------------------------------------------
  const focusEditor = (): void => ed.focus()
  const preferences = new Preferences(byId('preferences'), store, { version, defaultFontFamily: fontFamily, onClose: focusEditor })
  let commands: Command[] = []
  const palette = new Palette(byId('palette'), () => commands, platform, focusEditor)

  commands = createCommands(
    {
      openPreferences: () => {
        palette.close()
        preferences.open()
      },
      openPalette: () => {
        preferences.close()
        palette.open()
      },
      quit,
      toggleFullscreen: async () => appWindow.setFullscreen(!(await appWindow.isFullscreen())),
      changeFontSize: (delta) => store.set({ fontSize: clamp(store.state.fontSize + delta, FONT_SIZE_MIN, FONT_SIZE_MAX) }),
      toggleVim: () => store.set({ editorMode: store.state.editorMode === 'vim' ? 'normal' : 'vim' }),
      toggleAlwaysOnTop: () => store.set({ alwaysOnTop: !store.state.alwaysOnTop }),
      setTheme: (value) => store.set({ theme: value }),
      setLanguage: (language) => store.set({ language }),
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
      if (palette.isOpen) {
        event.preventDefault()
        palette.close()
      } else if (preferences.isOpen) {
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

  ed.focus()
}

main().catch((err: unknown) => {
  console.error(err)
  document.body.textContent = `draftpad の起動に失敗しました: ${String(err)}`
})
