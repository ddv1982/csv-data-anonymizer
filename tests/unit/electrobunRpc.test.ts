import { beforeEach, describe, expect, it, vi } from 'vitest'
import { defaultAppSettings } from '../../src/shared/contracts'
import { createCsvAnonymizerRpcHandlers } from '../../src/bun/rpc'
import { AnonymizerError, ErrorCodes } from '../../src/types/errors'

describe('createCsvAnonymizerRpcHandlers', () => {
  const service = {
    getHealth: vi.fn(() => ({ status: 'ok' as const, version: '1.0.1', timestamp: '2026-06-18T00:00:00.000Z' })),
    analyzeCsv: vi.fn(),
    previewAnonymization: vi.fn(),
    anonymizeCsv: vi.fn()
  }
  const settingsStore = {
    getSettings: vi.fn(() => defaultAppSettings),
    updateSettings: vi.fn((patch) => ({ ...defaultAppSettings, ...patch }))
  }
  const platform = {
    openFileDialog: vi.fn(),
    showItemInFolder: vi.fn()
  }

  beforeEach(() => {
    vi.clearAllMocks()
    settingsStore.getSettings.mockReturnValue(defaultAppSettings)
    platform.openFileDialog.mockResolvedValue([])
  })

  it('returns health through the ApiResult success envelope', async () => {
    const handlers = createCsvAnonymizerRpcHandlers(service as never, settingsStore as never, platform)

    await expect(handlers.getHealth(undefined)).resolves.toEqual({
      success: true,
      data: {
        status: 'ok',
        version: '1.0.1',
        timestamp: '2026-06-18T00:00:00.000Z'
      }
    })
  })

  it('selects a CSV file and remembers the chosen directory', async () => {
    const handlers = createCsvAnonymizerRpcHandlers(service as never, settingsStore as never, platform)
    platform.openFileDialog.mockResolvedValue(['/tmp/input/customers.csv'])

    await expect(handlers.selectCsvFile(undefined)).resolves.toEqual({
      success: true,
      data: { filePath: '/tmp/input/customers.csv' }
    })
    expect(platform.openFileDialog).toHaveBeenCalledWith({
      startingFolder: undefined,
      allowedFileTypes: 'csv',
      canChooseFiles: true,
      canChooseDirectory: false,
      allowsMultipleSelection: false
    })
    expect(settingsStore.updateSettings).toHaveBeenCalledWith({ files: { lastInputDirectory: '/tmp/input' } })
  })

  it('uses a directory chooser plus default filename for output selection', async () => {
    const handlers = createCsvAnonymizerRpcHandlers(service as never, settingsStore as never, platform)
    platform.openFileDialog.mockResolvedValue(['/tmp/output'])

    await expect(handlers.selectOutputFile({ defaultPath: '/tmp/input/customers_anonymized.csv' })).resolves.toEqual({
      success: true,
      data: { filePath: '/tmp/output/customers_anonymized.csv' }
    })
    expect(platform.openFileDialog).toHaveBeenCalledWith({
      startingFolder: '/tmp/input',
      canChooseFiles: false,
      canChooseDirectory: true,
      allowsMultipleSelection: false
    })
    expect(settingsStore.updateSettings).toHaveBeenCalledWith({ files: { lastOutputDirectory: '/tmp/output' } })
  })

  it('maps validation errors to CONFIG_INVALID failures', async () => {
    const handlers = createCsvAnonymizerRpcHandlers(service as never, settingsStore as never, platform)

    const response = await handlers.getHeaders({ filePath: '', sampleRows: 10 })

    expect(response.success).toBe(false)
    if (!response.success) {
      expect(response.error.code).toBe(ErrorCodes.CONFIG_INVALID)
      expect(response.error.suggestion).toBe('Check the selected settings and try again.')
    }
  })

  it('maps anonymizer errors without throwing over RPC', async () => {
    const handlers = createCsvAnonymizerRpcHandlers(service as never, settingsStore as never, platform)
    service.analyzeCsv.mockRejectedValueOnce(new AnonymizerError('Missing file', ErrorCodes.FILE_NOT_FOUND, 'Choose another file.'))

    await expect(handlers.getHeaders({ filePath: '/missing.csv', sampleRows: 10 })).resolves.toEqual({
      success: false,
      error: {
        code: ErrorCodes.FILE_NOT_FOUND,
        message: 'Missing file',
        suggestion: 'Choose another file.'
      }
    })
  })

  it('maps unknown errors to UNKNOWN failures', async () => {
    const handlers = createCsvAnonymizerRpcHandlers(service as never, settingsStore as never, platform)
    service.previewAnonymization.mockRejectedValueOnce(new Error('preview failed'))

    await expect(
      handlers.getPreview({
        filePath: '/tmp/input.csv',
        columns: [0],
        deterministic: false,
        sampleCount: 1
      })
    ).resolves.toEqual({
      success: false,
      error: {
        code: 'UNKNOWN',
        message: 'preview failed',
        suggestion: 'Try again or choose a different file.'
      }
    })
  })
})
