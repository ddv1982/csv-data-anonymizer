import { access } from 'node:fs/promises'
import { constants, createReadStream, existsSync } from 'node:fs'
import { dirname, isAbsolute, normalize, resolve } from 'node:path'
import Papa from 'papaparse'
import { readSample } from '../core/sampleReader.js'
import { getFatalParseError } from '../core/papaParseErrors.js'
import { buildColumnMetadata, applyColumnSelection } from '../core/metadataBuilder.js'
import { transformValue, createTransformContext } from '../core/transformer.js'
import { processFile } from '../core/processor.js'
import { AnonymizerError, ErrorCodes, OutputExistsError } from '../types/errors.js'
import type { ColumnMetadata } from '../types/column.js'
import type {
  AnonymizeData,
  AnonymizeParams,
  ColumnInfo,
  ColumnPreview,
  GetHeadersParams,
  GetPreviewParams,
  HeadersData,
  HealthData,
  PreviewData,
  SampleTransform
} from '../shared/contracts'

const defaultSampleRows = 100

export class AnonymizerService {
  constructor(private readonly version: string) {}

  getHealth(): HealthData {
    return {
      status: 'ok',
      version: this.version,
      timestamp: new Date().toISOString()
    }
  }

  async analyzeCsv(input: GetHeadersParams): Promise<HeadersData> {
    const normalizedPath = validateFilePath(input.filePath)
    const sample = await readSample(normalizedPath, input.sampleRows ?? defaultSampleRows)
    const metadata = buildColumnMetadata(sample.headers, sample.rows)
    const rowCount = await countCsvDataRows(normalizedPath).catch(() => sample.rows.length)

    return {
      filePath: normalizedPath,
      rowCount,
      defaultOutputPath: generateDefaultOutputPath(normalizedPath),
      columns: metadata.map(toColumnInfo)
    }
  }

  async previewAnonymization(input: GetPreviewParams): Promise<PreviewData> {
    const normalizedPath = validateFilePath(input.filePath)
    const sample = await readSample(normalizedPath, input.sampleCount * 2)
    const metadata = buildColumnMetadata(sample.headers, sample.rows)
    validateColumnIndices(metadata, input.columns)
    const selectedMetadata = applyColumnSelection(metadata, input.columns)
    const seed = input.seed ?? ''
    const previews = selectedMetadata
      .filter((column) => column.isSelected)
      .map((column) => generateColumnPreview(column, sample.rows, input.sampleCount, input.deterministic, seed))

    return { previews }
  }

  async anonymizeCsv(input: AnonymizeParams): Promise<AnonymizeData> {
    const normalizedInputPath = validateFilePath(input.filePath)
    const normalizedOutputPath = await validateOutputPath(input.outputPath, input.force)
    const sample = await readSample(normalizedInputPath, defaultSampleRows)
    const metadata = buildColumnMetadata(sample.headers, sample.rows)
    validateColumnIndices(metadata, input.columns)
    const selectedMetadata = applyColumnSelection(metadata, input.columns)
    const result = await processFile(normalizedInputPath, normalizedOutputPath, selectedMetadata, {
      deterministic: input.deterministic,
      seed: input.seed ?? ''
    })

    return {
      outputPath: normalizedOutputPath,
      rowCount: result.rowCount,
      columnsAnonymized: input.columns.length,
      duration: result.duration
    }
  }
}

export function generateDefaultOutputPath(inputPath: string): string {
  const lastDot = inputPath.lastIndexOf('.')
  const lastSlash = Math.max(inputPath.lastIndexOf('/'), inputPath.lastIndexOf('\\'))
  const baseName = lastDot > lastSlash ? inputPath.slice(0, lastDot) : inputPath
  const extension = lastDot > lastSlash ? inputPath.slice(lastDot) : '.csv'
  return `${baseName}_anonymized${extension}`
}

function validateFilePath(filePath: string): string {
  if (filePath.includes('\0')) {
    throw new AnonymizerError('Invalid file path: null bytes not allowed', ErrorCodes.FILE_NOT_FOUND)
  }

  return normalize(isAbsolute(filePath) ? filePath : resolve(process.cwd(), filePath))
}

async function validateOutputPath(outputPath: string, force: boolean): Promise<string> {
  const normalized = validateFilePath(outputPath)

  if (existsSync(normalized) && !force) {
    throw new OutputExistsError(normalized)
  }

  const outputDir = dirname(normalized)
  try {
    await access(outputDir, constants.W_OK)
  } catch {
    throw new AnonymizerError(
      `Output directory is not writable: ${outputDir}`,
      ErrorCodes.FILE_NOT_FOUND,
      'Choose an existing output directory with write permissions.'
    )
  }

  return normalized
}

function validateColumnIndices(metadata: ColumnMetadata[], columns: number[]): void {
  const maxColumnIndex = metadata.length - 1
  for (const columnIndex of columns) {
    if (columnIndex > maxColumnIndex) {
      throw new AnonymizerError(
        `Column index ${columnIndex} is out of range. Valid range: 0-${maxColumnIndex}`,
        ErrorCodes.COLUMN_NOT_FOUND,
        `Use column indices between 0 and ${maxColumnIndex}.`
      )
    }
  }
}

function toColumnInfo(column: ColumnMetadata): ColumnInfo {
  return {
    index: column.index,
    name: column.name,
    detectedType: column.detectedType,
    confidence: column.confidence,
    piiRisk: column.piiRisk,
    sampleValues: column.sampleValues,
    emptyFormat: column.emptyFormat
  }
}

function generateColumnPreview(
  column: ColumnMetadata,
  rows: string[][],
  sampleCount: number,
  deterministic: boolean,
  seed: string
): ColumnPreview {
  const samples: SampleTransform[] = []
  const columnValues: Array<{ value: string; rowIndex: number }> = []

  for (let rowIndex = 0; rowIndex < rows.length && columnValues.length < sampleCount; rowIndex++) {
    const value = rows[rowIndex][column.index]
    if (value && value.trim() !== '' && value.toLowerCase() !== 'null') {
      columnValues.push({ value, rowIndex })
    }
  }

  for (const { value, rowIndex } of columnValues) {
    const context = createTransformContext(column, rowIndex, seed, deterministic)
    samples.push({
      original: value,
      anonymized: transformValue(value, column, context)
    })
  }

  return {
    columnIndex: column.index,
    columnName: column.name,
    samples
  }
}

function countCsvDataRows(filePath: string): Promise<number> {
  return new Promise((resolve, reject) => {
    const fileStream = createReadStream(filePath, { encoding: 'utf-8' })
    let rowCount = 0
    let headerProcessed = false
    let settled = false

    const finish = (callback: () => void): void => {
      if (settled) return
      settled = true
      callback()
    }

    fileStream.on('error', (error) => {
      finish(() => reject(error))
    })

    Papa.parse<string[]>(fileStream, {
      header: false,
      skipEmptyLines: true,
      step: (results, parserInstance) => {
        const fatalError = getFatalParseError(results.errors)
        if (fatalError) {
          parserInstance.abort()
          finish(() => reject(new Error(fatalError.message)))
          return
        }

        if (!headerProcessed) {
          headerProcessed = true
          return
        }

        rowCount++
      },
      complete: () => {
        finish(() => resolve(rowCount))
      },
      error: (error: Error) => {
        finish(() => reject(error))
      }
    })
  })
}
