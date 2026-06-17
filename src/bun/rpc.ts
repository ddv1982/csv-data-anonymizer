import { basename, dirname, extname, join } from 'node:path'
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
  type AppSettingsPatch,
  type OutputPathDialogParams
} from '../shared/contracts'
import { AnonymizerError, ErrorCodes } from '../types/errors.js'
import type { AnonymizerService } from '../services/anonymizerService'
import type { SettingsStore } from '../services/settingsStore'
import type { CsvAnonymizerRpcSchema } from './rpc-schema'

type DialogOptions = {
  startingFolder?: string
  allowedFileTypes?: string
  canChooseFiles?: boolean
  canChooseDirectory?: boolean
  allowsMultipleSelection?: boolean
}

export interface CsvAnonymizerPlatformApi {
  openFileDialog(options?: DialogOptions): Promise<string[]>
  showItemInFolder(path: string): unknown
}

type RpcRequestHandlers = NonNullable<CsvAnonymizerRpcSchema['bun']['requests']>

export type CsvAnonymizerRpcHandlers = {
  [Method in keyof RpcRequestHandlers]: (
    params: RpcRequestHandlers[Method]['params']
  ) => Promise<RpcRequestHandlers[Method]['response']>
}

export function createCsvAnonymizerRpcHandlers(
  service: AnonymizerService,
  settingsStore: SettingsStore,
  platform: CsvAnonymizerPlatformApi
): CsvAnonymizerRpcHandlers {
  return {
    getHealth: () => result(() => service.getHealth()),
    getSettings: () => result(() => settingsStore.getSettings()),
    updateSettings: (input: AppSettingsPatch) => result(() => settingsStore.updateSettings(appSettingsPatchSchema.parse(input))),
    selectCsvFile: () =>
      result(async () => {
        const settings = settingsStore.getSettings()
        const selectedPaths = await platform.openFileDialog({
          startingFolder: settings.files.rememberLastPaths ? settings.files.lastInputDirectory ?? undefined : undefined,
          allowedFileTypes: 'csv',
          canChooseFiles: true,
          canChooseDirectory: false,
          allowsMultipleSelection: false
        })
        const filePath = firstSelectedPath(selectedPaths)

        if (filePath && settings.files.rememberLastPaths) {
          settingsStore.updateSettings({ files: { lastInputDirectory: dirname(filePath) } })
        }

        return { filePath }
      }),
    selectOutputFile: (input?: OutputPathDialogParams) =>
      result(async () => {
        const settings = settingsStore.getSettings()
        const parsed = outputPathDialogRequestSchema.parse(input ?? {})
        const defaultOutputPath = parsed.defaultPath
        const selectedPaths = await platform.openFileDialog({
          startingFolder:
            dirnameOrFallback(defaultOutputPath) ??
            (settings.files.rememberLastPaths ? settings.files.lastOutputDirectory ?? undefined : undefined),
          canChooseFiles: false,
          canChooseDirectory: true,
          allowsMultipleSelection: false
        })
        const selectedDirectory = firstSelectedPath(selectedPaths)
        const filePath = selectedDirectory ? join(selectedDirectory, outputFileName(defaultOutputPath)) : null

        if (filePath && settings.files.rememberLastPaths) {
          settingsStore.updateSettings({ files: { lastOutputDirectory: dirname(filePath) } })
        }

        return { filePath }
      }),
    showOutputInFolder: (input) =>
      result(() => {
        const { outputPath } = showItemRequestSchema.parse(input)
        platform.showItemInFolder(outputPath)
        return { completed: true }
      }),
    getHeaders: (input) => result(() => service.analyzeCsv(headersRequestSchema.parse(input))),
    getPreview: (input) => result(() => service.previewAnonymization(previewRequestSchema.parse(input))),
    anonymizeFile: (input) => result(() => service.anonymizeCsv(anonymizeRequestSchema.parse(input)))
  }
}

function firstSelectedPath(paths: string[]): string | null {
  return paths.find((path) => path.trim().length > 0) ?? null
}

function dirnameOrFallback(path: string | undefined): string | undefined {
  return path ? dirname(path) : undefined
}

function outputFileName(defaultPath: string | undefined): string {
  if (!defaultPath) return 'anonymized.csv'

  const fileName = basename(defaultPath)
  return extname(fileName) ? fileName : `${fileName}.csv`
}

export async function result<T>(operation: () => T | Promise<T>): Promise<ApiResult<T>> {
  try {
    return {
      success: true,
      data: await operation()
    }
  } catch (error) {
    return toApiFailure(error)
  }
}

export function toApiFailure(error: unknown): ApiFailure {
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
