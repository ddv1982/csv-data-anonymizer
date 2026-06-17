import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { app, BrowserWindow, screen, shell } from 'electron'
import type { Rectangle } from 'electron'
import { electronApp, is, optimizer } from '@electron-toolkit/utils'
import { registerIpcHandlers } from './ipc'
import { AnonymizerService } from './services/anonymizerService'
import { SettingsStore } from './services/settingsStore'

let mainWindow: BrowserWindow | null = null
let windowStatePath: string | null = null
let windowStateTimer: NodeJS.Timeout | null = null

const defaultWindowBounds = {
  width: 1180,
  height: 820
}

const minimumWindowBounds = {
  width: 760,
  height: 560
}

type StoredWindowBounds = Pick<Rectangle, 'x' | 'y' | 'width' | 'height'>

function createWindow(): void {
  const savedBounds = windowStatePath ? loadWindowBounds(windowStatePath) : null
  mainWindow = new BrowserWindow({
    ...defaultWindowBounds,
    ...savedBounds,
    minWidth: minimumWindowBounds.width,
    minHeight: minimumWindowBounds.height,
    resizable: true,
    show: false,
    titleBarStyle: process.platform === 'darwin' ? 'hiddenInset' : 'default',
    backgroundColor: '#09090b',
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false
    }
  })

  persistWindowBounds(mainWindow)
  mainWindow.on('ready-to-show', () => mainWindow?.show())
  mainWindow.webContents.on('did-fail-load', (_event, errorCode, errorDescription, validatedUrl) => {
    console.error(`Renderer failed to load ${validatedUrl}: ${errorCode} ${errorDescription}`)
    mainWindow?.show()
  })
  setTimeout(() => mainWindow?.show(), 3000)

  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    if (canOpenExternalUrl(url)) void shell.openExternal(url)
    return { action: 'deny' }
  })

  if (is.dev && process.env.ELECTRON_RENDERER_URL) {
    mainWindow.loadURL(process.env.ELECTRON_RENDERER_URL)
  } else {
    mainWindow.loadFile(join(__dirname, '../renderer/index.html'))
  }
}

function canOpenExternalUrl(value: string): boolean {
  try {
    const url = new URL(value)
    return url.protocol === 'https:' || url.protocol === 'http:' || url.protocol === 'mailto:'
  } catch {
    return false
  }
}

function loadWindowBounds(path: string): StoredWindowBounds | null {
  if (!existsSync(path)) return null

  try {
    const parsed = JSON.parse(readFileSync(path, 'utf8')) as Partial<StoredWindowBounds>
    if (!isValidWindowBounds(parsed)) return null
    return fitWindowBoundsToDisplay(parsed)
  } catch {
    return null
  }
}

function isValidWindowBounds(value: Partial<StoredWindowBounds>): value is StoredWindowBounds {
  return (
    Number.isFinite(value.x) &&
    Number.isFinite(value.y) &&
    Number.isFinite(value.width) &&
    Number.isFinite(value.height) &&
    Number(value.width) > 0 &&
    Number(value.height) > 0
  )
}

function fitWindowBoundsToDisplay(bounds: StoredWindowBounds): StoredWindowBounds {
  const display = screen.getDisplayMatching(bounds)
  const { workArea } = display
  const width = Math.min(
    Math.max(Math.round(bounds.width), minimumWindowBounds.width),
    Math.max(workArea.width, minimumWindowBounds.width)
  )
  const height = Math.min(
    Math.max(Math.round(bounds.height), minimumWindowBounds.height),
    Math.max(workArea.height, minimumWindowBounds.height)
  )
  const maxX = workArea.x + workArea.width - width
  const maxY = workArea.y + workArea.height - height

  return {
    x: clamp(Math.round(bounds.x), workArea.x, maxX),
    y: clamp(Math.round(bounds.y), workArea.y, maxY),
    width,
    height
  }
}

function clamp(value: number, min: number, max: number): number {
  if (max < min) return min
  return Math.min(Math.max(value, min), max)
}

function persistWindowBounds(window: BrowserWindow): void {
  if (!windowStatePath) return

  const scheduleSave = (): void => {
    if (window.isMinimized() || window.isFullScreen()) return
    if (windowStateTimer) clearTimeout(windowStateTimer)
    windowStateTimer = setTimeout(() => saveWindowBounds(window), 250)
  }

  window.on('move', scheduleSave)
  window.on('resize', scheduleSave)
  window.on('close', () => {
    if (windowStateTimer) clearTimeout(windowStateTimer)
    saveWindowBounds(window)
  })
}

function saveWindowBounds(window: BrowserWindow): void {
  if (!windowStatePath || window.isDestroyed() || window.isMinimized() || window.isFullScreen()) return
  const bounds = window.getNormalBounds()
  mkdirSync(dirname(windowStatePath), { recursive: true })
  writeFileSync(windowStatePath, `${JSON.stringify(bounds, null, 2)}\n`)
}

app.whenReady().then(() => {
  electronApp.setAppUserModelId('com.csv-anonymizer.app')
  app.on('browser-window-created', (_, window) => optimizer.watchWindowShortcuts(window))

  const userDataPath = app.getPath('userData')
  windowStatePath = join(userDataPath, 'window-state.json')
  const settingsStore = new SettingsStore(userDataPath)
  const service = new AnonymizerService(app.getVersion())
  registerIpcHandlers(service, settingsStore)

  createWindow()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow()
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})
