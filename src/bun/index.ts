import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import {
  ApplicationMenu,
  BrowserView,
  BrowserWindow,
  Screen,
  Updater,
  Utils,
  type ApplicationMenuItemConfig,
  type Display,
  type Rectangle
} from 'electrobun/bun'
import { AnonymizerService } from '../services/anonymizerService'
import { SettingsStore } from '../services/settingsStore'
import { createCsvAnonymizerRpcHandlers, type DialogOptions } from './rpc'
import type { CsvAnonymizerRpcSchema } from './rpc-schema'
import { startSmokeServer } from './smoke-server'

const appName = 'CSV Anonymizer'
const appIdentifier = 'com.csv-anonymizer.app'
const defaultRendererUrl = 'views://mainview/index.html'
const rpcTimeoutMs = 5 * 60 * 1000

let mainWindow: BrowserWindow | null = null
let windowStatePath: string | null = null
let windowStateTimer: ReturnType<typeof setTimeout> | null = null

const defaultWindowBounds = {
  width: 1180,
  height: 820
}

const minimumWindowBounds = {
  width: 760,
  height: 560
}

type StoredWindowBounds = Pick<Rectangle, 'x' | 'y' | 'width' | 'height'>

const userDataPath = getUserDataPath()
windowStatePath = join(userDataPath, 'window-state.json')

const settingsStore = new SettingsStore(userDataPath)
const appVersion = await getAppVersion()
const service = new AnonymizerService(appVersion)
const rpcHandlers = createCsvAnonymizerRpcHandlers(service, settingsStore, {
  openFileDialog: openFileDialogSafely,
  showItemInFolder: Utils.showItemInFolder
})
const rpc = BrowserView.defineRPC<CsvAnonymizerRpcSchema>({
  maxRequestTime: rpcTimeoutMs,
  handlers: {
    requests: rpcHandlers
  }
})
setupApplicationMenu()
createWindow()
startSmokeServer(rpcHandlers)

function setupApplicationMenu(): void {
  const menu: ApplicationMenuItemConfig[] = [
    {
      label: appName,
      submenu: [
        { role: 'about' },
        { type: 'divider' },
        { role: 'hide' },
        { role: 'hideOthers' },
        { role: 'showAll' },
        { type: 'divider' },
        { role: 'quit' }
      ]
    },
    {
      label: 'File',
      submenu: [{ role: 'close' }]
    },
    {
      label: 'Edit',
      submenu: [
        { role: 'undo' },
        { role: 'redo' },
        { type: 'divider' },
        { role: 'cut' },
        { role: 'copy' },
        { role: 'paste' },
        { role: 'pasteAndMatchStyle' },
        { role: 'delete' },
        { type: 'divider' },
        { role: 'selectAll' }
      ]
    },
    {
      label: 'View',
      submenu: [{ role: 'toggleFullScreen' }]
    },
    {
      label: 'Window',
      submenu: [
        { role: 'minimize' },
        { role: 'zoom' },
        { type: 'divider' },
        { role: 'bringAllToFront' }
      ]
    }
  ]

  ApplicationMenu.setApplicationMenu(menu)
}

function createWindow(): void {
  const savedBounds = windowStatePath ? loadWindowBounds(windowStatePath) : null
  const initialFrame = savedBounds ?? {
    x: 80,
    y: 80,
    ...defaultWindowBounds
  }

  mainWindow = new BrowserWindow({
    title: appName,
    frame: initialFrame,
    url: process.env.CSV_ANONYMIZER_RENDERER_URL ?? defaultRendererUrl,
    rpc,
    hidden: true,
    titleBarStyle: 'default',
    transparent: false,
    passthrough: false,
    sandbox: false
  })

  persistWindowBounds(mainWindow)
  mainWindow.webview.on('dom-ready', () => mainWindow?.show())
  mainWindow.webview.on('new-window-open', (event) => openAllowedExternalUrl(event))
  mainWindow.webview.on('will-navigate', (event) => openExternalNavigation(event))
  setTimeout(() => mainWindow?.show(), 3000)
}

function getUserDataPath(): string {
  const configuredPath = Utils.paths.userData
  const fallbackPath = join(Utils.paths.appData, appIdentifier, process.env.ELECTROBUN_BUILD_ENV ?? 'dev')
  return configuredPath === Utils.paths.appData ? fallbackPath : configuredPath
}

async function getAppVersion(): Promise<string> {
  try {
    return (await Updater.localInfo.version()) || '0.0.0'
  } catch {
    return '0.0.0'
  }
}

async function openFileDialogSafely(options?: DialogOptions): Promise<string[]> {
  try {
    const paths = await Utils.openFileDialog(options)
    return Array.isArray(paths) ? paths : []
  } catch (error) {
    if (isFileDialogCancellationError(error)) return []
    throw error
  }
}

function isFileDialogCancellationError(error: unknown): boolean {
  if (!(error instanceof Error)) return false
  return /Cannot read properties of (null|undefined) \(reading 'split'\)/.test(error.message)
}

function canOpenExternalUrl(value: string): boolean {
  try {
    const url = new URL(value)
    return url.protocol === 'https:' || url.protocol === 'http:' || url.protocol === 'mailto:'
  } catch {
    return false
  }
}

function isAppUrl(value: string): boolean {
  try {
    const url = new URL(value)
    return url.protocol === 'views:'
  } catch {
    return false
  }
}

function openAllowedExternalUrl(event: unknown): void {
  const url = extractEventUrl(event)
  if (url && canOpenExternalUrl(url)) Utils.openExternal(url)
}

function openExternalNavigation(event: unknown): void {
  const url = extractEventUrl(event)
  if (url && !isAppUrl(url) && canOpenExternalUrl(url)) Utils.openExternal(url)
}

function extractEventUrl(event: unknown): string | null {
  const data = event && typeof event === 'object' && 'data' in event ? (event as { data?: unknown }).data : event
  const detail = data && typeof data === 'object' && 'detail' in data ? (data as { detail?: unknown }).detail : data

  if (typeof detail === 'string') return detail
  if (detail && typeof detail === 'object' && 'url' in detail && typeof (detail as { url?: unknown }).url === 'string') {
    return (detail as { url: string }).url
  }

  return null
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
  const display = getDisplayMatching(bounds)
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

function getDisplayMatching(bounds: StoredWindowBounds): Display {
  const displays = Screen.getAllDisplays()
  const primaryDisplay = Screen.getPrimaryDisplay()
  if (displays.length === 0) return primaryDisplay

  let bestDisplay = displays[0] ?? primaryDisplay
  let bestArea = -1
  for (const display of displays) {
    const area = intersectionArea(bounds, display.workArea)
    if (area > bestArea) {
      bestDisplay = display
      bestArea = area
    }
  }

  return bestArea > 0 ? bestDisplay : primaryDisplay
}

function intersectionArea(a: Rectangle, b: Rectangle): number {
  const xOverlap = Math.max(0, Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x))
  const yOverlap = Math.max(0, Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y))
  return xOverlap * yOverlap
}

function clamp(value: number, min: number, max: number): number {
  if (max < min) return min
  return Math.min(Math.max(value, min), max)
}

function persistWindowBounds(window: BrowserWindow): void {
  if (!windowStatePath) return

  const scheduleSave = (): void => {
    enforceMinimumWindowSize(window)
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

function enforceMinimumWindowSize(window: BrowserWindow): void {
  const frame = window.getFrame()
  const width = Math.max(frame.width, minimumWindowBounds.width)
  const height = Math.max(frame.height, minimumWindowBounds.height)
  if (width !== frame.width || height !== frame.height) {
    window.setFrame(frame.x, frame.y, width, height)
  }
}

function saveWindowBounds(window: BrowserWindow): void {
  if (!windowStatePath || window.isMinimized() || window.isFullScreen()) return
  const bounds = window.getFrame()
  mkdirSync(dirname(windowStatePath), { recursive: true })
  writeFileSync(windowStatePath, `${JSON.stringify(bounds, null, 2)}\n`)
}
