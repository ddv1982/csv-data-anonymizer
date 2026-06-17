import { dirname } from 'node:path'
import { dialog, ipcMain, shell } from 'electron'
import { z } from 'zod'
import {
  anonymizeRequestSchema,
  appSettingsPatchSchema,
  headersRequestSchema,
  outputPathDialogRequestSchema,
  previewRequestSchema,
  showItemRequestSchema,
  type ApiFailure,
  type ApiResult,
  type AppSettingsPatch
} from '../shared/contracts'
import { AnonymizerError, ErrorCodes } from '../types/errors.js'
import type { AnonymizerService } from './services/anonymizerService'
import type { SettingsStore } from './services/settingsStore'

export function registerIpcHandlers(service: AnonymizerService, settingsStore: SettingsStore): void {
  ipcMain.handle('app:health', () => result(() => service.getHealth()))
  ipcMain.handle('settings:get', () => result(() => settingsStore.getSettings()))
  ipcMain.handle('settings:update', (_event, input: AppSettingsPatch) =>
    result(() => settingsStore.updateSettings(appSettingsPatchSchema.parse(input)))
  )
  ipcMain.handle('dialog:select-csv', () =>
    result(async () => {
      const settings = settingsStore.getSettings()
      const dialogResult = await dialog.showOpenDialog({
        title: 'Select CSV File',
        defaultPath: settings.files.rememberLastPaths ? settings.files.lastInputDirectory ?? undefined : undefined,
        properties: ['openFile'],
        filters: [{ name: 'CSV files', extensions: ['csv'] }]
      })

      const filePath = dialogResult.canceled ? null : dialogResult.filePaths[0] ?? null
      if (filePath && settings.files.rememberLastPaths) {
        settingsStore.updateSettings({ files: { lastInputDirectory: dirname(filePath) } })
      }

      return { filePath }
    })
  )
  ipcMain.handle('dialog:select-output', (_event, input) =>
    result(async () => {
      const settings = settingsStore.getSettings()
      const parsed = outputPathDialogRequestSchema.parse(input ?? {})
      const dialogResult = await dialog.showSaveDialog({
        title: 'Choose Output CSV',
        defaultPath:
          parsed.defaultPath ?? (settings.files.rememberLastPaths ? settings.files.lastOutputDirectory ?? undefined : undefined),
        filters: [{ name: 'CSV files', extensions: ['csv'] }]
      })

      const filePath = dialogResult.canceled ? null : dialogResult.filePath ?? null
      if (filePath && settings.files.rememberLastPaths) {
        settingsStore.updateSettings({ files: { lastOutputDirectory: dirname(filePath) } })
      }

      return { filePath }
    })
  )
  ipcMain.handle('shell:show-output', (_event, input) =>
    result(() => {
      const { outputPath } = showItemRequestSchema.parse(input)
      shell.showItemInFolder(outputPath)
      return { completed: true }
    })
  )
  ipcMain.handle('csv:headers', (_event, input) =>
    result(() => service.analyzeCsv(headersRequestSchema.parse(input)))
  )
  ipcMain.handle('csv:preview', (_event, input) =>
    result(() => service.previewAnonymization(previewRequestSchema.parse(input)))
  )
  ipcMain.handle('csv:anonymize', (_event, input) =>
    result(() => service.anonymizeCsv(anonymizeRequestSchema.parse(input)))
  )
}

async function result<T>(operation: () => T | Promise<T>): Promise<ApiResult<T>> {
  try {
    return {
      success: true,
      data: await operation()
    }
  } catch (error) {
    return toApiFailure(error)
  }
}

function toApiFailure(error: unknown): ApiFailure {
  if (error instanceof AnonymizerError) {
    return {
      success: false,
      error: {
        code: error.code,
        message: error.message,
        suggestion: error.suggestion
      }
    }
  }

  if (error instanceof z.ZodError) {
    return {
      success: false,
      error: {
        code: ErrorCodes.CONFIG_INVALID,
        message: z.prettifyError(error),
        suggestion: 'Check the selected settings and try again.'
      }
    }
  }

  return {
    success: false,
    error: {
      code: 'UNKNOWN',
      message: error instanceof Error ? error.message : 'Unexpected application error',
      suggestion: 'Try again or choose a different file.'
    }
  }
}
