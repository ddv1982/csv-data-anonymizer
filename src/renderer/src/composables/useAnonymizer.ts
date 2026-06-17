import { ref, computed } from 'vue'
import {
  anonymizeFile,
  getErrorMessage,
  getHeaders,
  getPreview,
  getSettings,
  isApiError,
  updateSettings,
  type AppSettings,
  type ColumnInfo,
  type ColumnPreview,
} from '@/lib/api'

export interface AnonymizeConfig {
  outputPath: string
  deterministic: boolean
  seed: string
  overwriteOutput: boolean
}

export interface AnonymizeResult {
  outputPath: string
  rowCount: number
  columnsAnonymized: number
  duration: number
}

export function useAnonymizer() {
  const selectedFile = ref<string | null>(null)
  const columns = ref<ColumnInfo[]>([])
  const selectedColumns = ref<number[]>([])
  const rowCount = ref(0)
  const config = ref<AnonymizeConfig>({
    outputPath: '',
    deterministic: false,
    seed: '',
    overwriteOutput: true,
  })
  const settings = ref<AppSettings | null>(null)
  const previews = ref<ColumnPreview[]>([])
  const progress = ref(0)
  const result = ref<AnonymizeResult | null>(null)
  const previewSeed = ref<string>('')

  const isLoadingSettings = ref(false)
  const isLoadingHeaders = ref(false)
  const isLoadingPreview = ref(false)
  const isAnonymizing = ref(false)

  const error = ref<string | null>(null)

  const hasFile = computed(() => selectedFile.value !== null)
  const hasColumns = computed(() => columns.value.length > 0)
  const hasSelectedColumns = computed(() => selectedColumns.value.length > 0)
  const canPreview = computed(
    () => hasFile.value && hasSelectedColumns.value && !isLoadingPreview.value
  )
  const canAnonymize = computed(
    () =>
      hasFile.value &&
      hasSelectedColumns.value &&
      config.value.outputPath.trim() !== '' &&
      !isAnonymizing.value
  )
  const isLoading = computed(
    () => isLoadingSettings.value || isLoadingHeaders.value || isLoadingPreview.value || isAnonymizing.value
  )
  const hasResult = computed(() => result.value !== null)

  function generateSeed(): string {
    return Math.random().toString(36).slice(2, 15)
  }

  function getSelectedColumnPayload(): number[] {
    return [...selectedColumns.value]
  }

  async function loadSettings(): Promise<void> {
    isLoadingSettings.value = true
    const response = await getSettings()

    if (isApiError(response)) {
      error.value = getErrorMessage(response)
    } else {
      settings.value = response.data
      config.value.deterministic = response.data.anonymization.deterministicDefault
      config.value.seed = response.data.anonymization.seed
      config.value.overwriteOutput = response.data.anonymization.overwriteOutput
    }

    isLoadingSettings.value = false
  }

  async function persistSettingsPatch(patch: Parameters<typeof updateSettings>[0]): Promise<void> {
    const response = await updateSettings(patch)
    if (isApiError(response)) {
      error.value = getErrorMessage(response)
      return
    }

    settings.value = response.data
  }

  function setDeterministic(value: boolean): void {
    config.value.deterministic = value
    void persistSettingsPatch({ anonymization: { deterministicDefault: value } })
  }

  function setSeed(value: string): void {
    config.value.seed = value
    void persistSettingsPatch({ anonymization: { seed: value } })
  }

  function setOverwriteOutput(value: boolean): void {
    config.value.overwriteOutput = value
    void persistSettingsPatch({ anonymization: { overwriteOutput: value } })
  }

  function setOutputPath(value: string): void {
    config.value.outputPath = value
  }

  async function loadHeaders(): Promise<void> {
    if (!selectedFile.value) {
      error.value = 'No file selected'
      return
    }

    isLoadingHeaders.value = true
    error.value = null

    const response = await getHeaders({
      filePath: selectedFile.value,
      sampleRows: settings.value?.anonymization.sampleRowCount,
    })

    if (isApiError(response)) {
      error.value = getErrorMessage(response)
      columns.value = []
      selectedColumns.value = []
      rowCount.value = 0
    } else {
      columns.value = response.data.columns
      rowCount.value = response.data.rowCount
      selectedColumns.value = response.data.columns
        .filter((col) => col.sampleValues.length > 0 && (col.piiRisk === 'high' || col.piiRisk === 'medium'))
        .map((col) => col.index)
      config.value.outputPath = response.data.defaultOutputPath
    }

    isLoadingHeaders.value = false
  }

  async function generatePreview(): Promise<void> {
    if (!selectedFile.value || selectedColumns.value.length === 0) {
      error.value = 'No file or columns selected'
      return
    }

    isLoadingPreview.value = true
    error.value = null

    previewSeed.value = config.value.deterministic
      ? config.value.seed || generateSeed()
      : generateSeed()

    if (config.value.deterministic && !config.value.seed) {
      setSeed(previewSeed.value)
    }

    const response = await getPreview({
      filePath: selectedFile.value,
      columns: getSelectedColumnPayload(),
      deterministic: config.value.deterministic,
      seed: previewSeed.value,
      sampleCount: settings.value?.anonymization.previewSampleCount ?? 5,
    })

    if (isApiError(response)) {
      error.value = getErrorMessage(response)
      previews.value = []
    } else {
      previews.value = response.data.previews
    }

    isLoadingPreview.value = false
  }

  async function runAnonymize(): Promise<void> {
    if (!selectedFile.value || selectedColumns.value.length === 0) {
      error.value = 'No file or columns selected'
      return
    }

    if (!config.value.outputPath.trim()) {
      error.value = 'Output path is required'
      return
    }

    isAnonymizing.value = true
    error.value = null
    progress.value = 0
    result.value = null

    const seedToUse = config.value.deterministic
      ? config.value.seed || previewSeed.value || generateSeed()
      : ''

    if (config.value.deterministic && !config.value.seed) {
      setSeed(seedToUse)
    }

    const response = await anonymizeFile({
      filePath: selectedFile.value,
      outputPath: config.value.outputPath,
      columns: getSelectedColumnPayload(),
      deterministic: config.value.deterministic,
      seed: seedToUse,
      force: config.value.overwriteOutput,
    })

    if (isApiError(response)) {
      error.value = getErrorMessage(response)
    } else {
      result.value = {
        outputPath: response.data.outputPath,
        rowCount: response.data.rowCount,
        columnsAnonymized: response.data.columnsAnonymized,
        duration: response.data.duration,
      }
      progress.value = 100
    }

    isAnonymizing.value = false
  }

  function reset(): void {
    selectedFile.value = null
    columns.value = []
    selectedColumns.value = []
    rowCount.value = 0
    config.value.outputPath = ''
    previews.value = []
    progress.value = 0
    result.value = null
    error.value = null
    previewSeed.value = ''
  }

  function clearError(): void {
    error.value = null
  }

  function setError(message: string): void {
    error.value = message
  }

  function setFile(filePath: string | null): void {
    if (filePath !== selectedFile.value) {
      columns.value = []
      selectedColumns.value = []
      rowCount.value = 0
      previews.value = []
      result.value = null
      previewSeed.value = ''
      error.value = null
      selectedFile.value = filePath
      config.value.outputPath = ''
    }
  }

  return {
    selectedFile,
    columns,
    selectedColumns,
    rowCount,
    config,
    settings,
    previews,
    progress,
    result,
    error,

    isLoadingSettings,
    isLoadingHeaders,
    isLoadingPreview,
    isAnonymizing,

    hasFile,
    hasColumns,
    hasSelectedColumns,
    canPreview,
    canAnonymize,
    isLoading,
    hasResult,

    loadSettings,
    loadHeaders,
    generatePreview,
    runAnonymize,
    reset,
    clearError,
    setError,
    setFile,
    setOutputPath,
    setDeterministic,
    setSeed,
    setOverwriteOutput,
  }
}
