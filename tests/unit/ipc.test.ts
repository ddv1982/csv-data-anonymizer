import { beforeEach, describe, expect, it, vi } from 'vitest'
import { defaultAppSettings } from '../../src/shared/contracts'
import { registerIpcHandlers } from '../../src/main/ipc'

const electronMocks = vi.hoisted(() => {
  const handlers = new Map<string, (...args: unknown[]) => unknown>()

  return {
    handlers,
    fromWebContents: vi.fn(),
    handle: vi.fn((channel: string, handler: (...args: unknown[]) => unknown) => {
      handlers.set(channel, handler)
    }),
    showOpenDialog: vi.fn(),
    showSaveDialog: vi.fn(),
    showItemInFolder: vi.fn(),
  }
})

vi.mock('electron', () => ({
  BrowserWindow: {
    fromWebContents: electronMocks.fromWebContents,
  },
  dialog: {
    showOpenDialog: electronMocks.showOpenDialog,
    showSaveDialog: electronMocks.showSaveDialog,
  },
  ipcMain: {
    handle: electronMocks.handle,
  },
  shell: {
    showItemInFolder: electronMocks.showItemInFolder,
  },
}))

describe('registerIpcHandlers dialog handlers', () => {
  const settingsStore = {
    getSettings: vi.fn(() => defaultAppSettings),
    updateSettings: vi.fn(() => defaultAppSettings),
  }

  beforeEach(() => {
    vi.clearAllMocks()
    electronMocks.handlers.clear()
    electronMocks.showOpenDialog.mockResolvedValue({ canceled: true, filePaths: [] })
    electronMocks.showSaveDialog.mockResolvedValue({ canceled: true, filePath: '' })

    registerIpcHandlers({} as never, settingsStore as never)
  })

  it('opens the CSV selector as a modal child of the invoking window', async () => {
    const parentWindow = createParentWindow()
    const sender = {}
    electronMocks.fromWebContents.mockReturnValue(parentWindow)

    const result = await invokeHandler('dialog:select-csv', { sender })

    expect(result).toEqual({ success: true, data: { filePath: null } })
    expect(electronMocks.fromWebContents).toHaveBeenCalledWith(sender)
    expect(parentWindow.focus).toHaveBeenCalledOnce()
    expect(electronMocks.showOpenDialog).toHaveBeenCalledWith(
      parentWindow,
      expect.objectContaining({
        title: 'Select CSV File',
        properties: ['openFile'],
      })
    )
  })

  it('restores and shows the invoking window before opening the CSV selector', async () => {
    const parentWindow = createParentWindow({
      isMinimized: vi.fn(() => true),
      isVisible: vi.fn(() => false),
    })
    electronMocks.fromWebContents.mockReturnValue(parentWindow)

    await invokeHandler('dialog:select-csv', { sender: {} })

    expect(parentWindow.restore).toHaveBeenCalledOnce()
    expect(parentWindow.show).toHaveBeenCalledOnce()
    expect(parentWindow.focus).toHaveBeenCalledOnce()
  })

  it('opens the output selector as a modal child of the invoking window', async () => {
    const parentWindow = createParentWindow()
    electronMocks.fromWebContents.mockReturnValue(parentWindow)

    await invokeHandler('dialog:select-output', { sender: {} }, { defaultPath: '/tmp/output.csv' })

    expect(electronMocks.showSaveDialog).toHaveBeenCalledWith(
      parentWindow,
      expect.objectContaining({
        title: 'Choose Output CSV',
        defaultPath: '/tmp/output.csv',
      })
    )
  })
})

function createParentWindow(overrides: Record<string, unknown> = {}) {
  return {
    isDestroyed: vi.fn(() => false),
    isMinimized: vi.fn(() => false),
    restore: vi.fn(),
    isVisible: vi.fn(() => true),
    show: vi.fn(),
    focus: vi.fn(),
    ...overrides,
  }
}

async function invokeHandler(channel: string, ...args: unknown[]): Promise<unknown> {
  const handler = electronMocks.handlers.get(channel)
  if (!handler) throw new Error(`Handler not registered: ${channel}`)

  return handler(...args)
}
