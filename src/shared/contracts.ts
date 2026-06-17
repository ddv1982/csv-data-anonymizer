import { z } from 'zod'

export const dataTypeSchema = z.enum([
  'email',
  'uuid',
  'timestamp',
  'numeric_id',
  'country_code',
  'phone',
  'first_name',
  'last_name',
  'full_name',
  'enum',
  'string',
  'unknown'
])
export type DataType = z.infer<typeof dataTypeSchema>

export const confidenceSchema = z.enum(['high', 'medium', 'low'])
export type Confidence = z.infer<typeof confidenceSchema>

export const piiRiskSchema = z.enum(['high', 'medium', 'low'])
export type PiiRisk = z.infer<typeof piiRiskSchema>

export const emptyFormatSchema = z.enum(['empty_string', 'null', 'mixed'])
export type EmptyFormat = z.infer<typeof emptyFormatSchema>

export const columnInfoSchema = z.object({
  index: z.number().int().min(0),
  name: z.string(),
  detectedType: dataTypeSchema,
  confidence: confidenceSchema,
  piiRisk: piiRiskSchema,
  sampleValues: z.array(z.string()),
  emptyFormat: emptyFormatSchema
})
export type ColumnInfo = z.infer<typeof columnInfoSchema>

export const apiErrorSchema = z.object({
  code: z.string(),
  message: z.string(),
  suggestion: z.string().optional()
})
export type ApiErrorDetails = z.infer<typeof apiErrorSchema>

export interface ApiSuccess<T> {
  success: true
  data: T
}

export interface ApiFailure {
  success: false
  error: ApiErrorDetails
}

export type ApiResult<T> = ApiSuccess<T> | ApiFailure

export const appSettingsSchemaVersion = 1

export const appSettingsSchema = z.object({
  schemaVersion: z.literal(appSettingsSchemaVersion),
  anonymization: z.object({
    deterministicDefault: z.boolean(),
    seed: z.string().max(256),
    overwriteOutput: z.boolean(),
    sampleRowCount: z.number().int().min(10).max(1000),
    previewSampleCount: z.number().int().min(1).max(20)
  }),
  files: z.object({
    defaultOutputSuffix: z.string().min(1).max(80),
    rememberLastPaths: z.boolean(),
    lastInputDirectory: z.string().nullable(),
    lastOutputDirectory: z.string().nullable()
  })
})
export type AppSettings = z.infer<typeof appSettingsSchema>

export const appSettingsPatchSchema = z.object({
  anonymization: z
    .object({
      deterministicDefault: z.boolean().optional(),
      seed: z.string().max(256).optional(),
      overwriteOutput: z.boolean().optional(),
      sampleRowCount: z.number().int().min(10).max(1000).optional(),
      previewSampleCount: z.number().int().min(1).max(20).optional()
    })
    .optional(),
  files: z
    .object({
      defaultOutputSuffix: z.string().min(1).max(80).optional(),
      rememberLastPaths: z.boolean().optional(),
      lastInputDirectory: z.string().nullable().optional(),
      lastOutputDirectory: z.string().nullable().optional()
    })
    .optional()
})
export type AppSettingsPatch = z.infer<typeof appSettingsPatchSchema>

export const defaultAppSettings: AppSettings = {
  schemaVersion: appSettingsSchemaVersion,
  anonymization: {
    deterministicDefault: false,
    seed: '',
    overwriteOutput: true,
    sampleRowCount: 100,
    previewSampleCount: 5
  },
  files: {
    defaultOutputSuffix: '_anonymized',
    rememberLastPaths: true,
    lastInputDirectory: null,
    lastOutputDirectory: null
  }
}

export const healthDataSchema = z.object({
  status: z.literal('ok'),
  version: z.string(),
  timestamp: z.string()
})
export type HealthData = z.infer<typeof healthDataSchema>

export const headersRequestSchema = z.object({
  filePath: z.string().min(1),
  sampleRows: z.number().int().min(10).max(1000).optional()
})
export type GetHeadersParams = z.infer<typeof headersRequestSchema>

export const headersDataSchema = z.object({
  filePath: z.string(),
  rowCount: z.number().int().min(0),
  defaultOutputPath: z.string(),
  columns: z.array(columnInfoSchema)
})
export type HeadersData = z.infer<typeof headersDataSchema>

export const sampleTransformSchema = z.object({
  original: z.string(),
  anonymized: z.string()
})
export type SampleTransform = z.infer<typeof sampleTransformSchema>

export const columnPreviewSchema = z.object({
  columnIndex: z.number().int().min(0),
  columnName: z.string(),
  samples: z.array(sampleTransformSchema)
})
export type ColumnPreview = z.infer<typeof columnPreviewSchema>

export const previewRequestSchema = z.object({
  filePath: z.string().min(1),
  columns: z.array(z.number().int().min(0)).min(1),
  deterministic: z.boolean().default(false),
  seed: z.string().optional(),
  sampleCount: z.number().int().min(1).max(20).default(defaultAppSettings.anonymization.previewSampleCount)
})
export type GetPreviewParams = z.infer<typeof previewRequestSchema>

export const previewDataSchema = z.object({
  previews: z.array(columnPreviewSchema)
})
export type PreviewData = z.infer<typeof previewDataSchema>

export const anonymizeRequestSchema = z.object({
  filePath: z.string().min(1),
  outputPath: z.string().min(1),
  columns: z.array(z.number().int().min(0)).min(1),
  deterministic: z.boolean().default(false),
  seed: z.string().optional(),
  force: z.boolean().default(false)
})
export type AnonymizeParams = z.infer<typeof anonymizeRequestSchema>

export const anonymizeDataSchema = z.object({
  outputPath: z.string(),
  rowCount: z.number().int().min(0),
  columnsAnonymized: z.number().int().min(0),
  duration: z.number().min(0)
})
export type AnonymizeData = z.infer<typeof anonymizeDataSchema>

export const fileDialogDataSchema = z.object({
  filePath: z.string().nullable()
})
export type FileDialogData = z.infer<typeof fileDialogDataSchema>

export const outputPathDialogRequestSchema = z.object({
  defaultPath: z.string().optional()
})
export type OutputPathDialogParams = z.infer<typeof outputPathDialogRequestSchema>

export const showItemRequestSchema = z.object({
  outputPath: z.string().min(1)
})
export type ShowItemParams = z.infer<typeof showItemRequestSchema>

export const actionDataSchema = z.object({
  completed: z.boolean()
})
export type ActionData = z.infer<typeof actionDataSchema>

export type HealthResponse = ApiSuccess<HealthData>
export type HeadersResponse = ApiSuccess<HeadersData>
export type PreviewResponse = ApiSuccess<PreviewData>
export type AnonymizeResponse = ApiSuccess<AnonymizeData>
export type SettingsResponse = ApiSuccess<AppSettings>
export type FileDialogResponse = ApiSuccess<FileDialogData>
export type ActionResponse = ApiSuccess<ActionData>

export interface CsvAnonymizerApi {
  getHealth(): Promise<ApiResult<HealthData>>
  getSettings(): Promise<ApiResult<AppSettings>>
  updateSettings(input: AppSettingsPatch): Promise<ApiResult<AppSettings>>
  selectCsvFile(): Promise<ApiResult<FileDialogData>>
  selectOutputFile(input?: OutputPathDialogParams): Promise<ApiResult<FileDialogData>>
  showOutputInFolder(input: ShowItemParams): Promise<ApiResult<ActionData>>
  getHeaders(input: GetHeadersParams): Promise<ApiResult<HeadersData>>
  getPreview(input: GetPreviewParams): Promise<ApiResult<PreviewData>>
  anonymizeFile(input: AnonymizeParams): Promise<ApiResult<AnonymizeData>>
}
