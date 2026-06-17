import { ref, computed } from 'vue'
import {
  getHeaders,
  getPreview,
  anonymizeFile,
  isApiError,
  getErrorMessage,
  type ColumnInfo,
  type ColumnPreview,
} from '@/lib/api'

export interface AnonymizeConfig {
  outputPath: string
  deterministic: boolean
  seed: string
}

export interface AnonymizeResult {
  outputPath: string
  rowCount: number
  columnsAnonymized: number
  duration: number
}

export function useAnonymizer() {
  // State
  const selectedFile = ref<string | null>(null)
  const columns = ref<ColumnInfo[]>([])
  const selectedColumns = ref<number[]>([])
  const rowCount = ref(0)
  const config = ref<AnonymizeConfig>({
    outputPath: '',
    deterministic: false,
    seed: '',
  })
  const previews = ref<ColumnPreview[]>([])
  const progress = ref(0)
  const result = ref<AnonymizeResult | null>(null)
  const previewSeed = ref<string>('')

  // Loading states
  const isLoadingHeaders = ref(false)
  const isLoadingPreview = ref(false)
  const isAnonymizing = ref(false)

  // Error state
  const error = ref<string | null>(null)

  // Computed
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
    () => isLoadingHeaders.value || isLoadingPreview.value || isAnonymizing.value
  )
  const hasResult = computed(() => result.value !== null)

  // Helper to generate a random seed
  function generateSeed(): string {
    return Math.random().toString(36).substring(2, 15)
  }

  // Helper to generate default output path
  function generateDefaultOutputPath(inputPath: string): string {
    const lastDot = inputPath.lastIndexOf('.')
    const lastSlash = Math.max(inputPath.lastIndexOf('/'), inputPath.lastIndexOf('\\'))
    const baseName = lastDot > lastSlash ? inputPath.slice(0, lastDot) : inputPath
    const ext = lastDot > lastSlash ? inputPath.slice(lastDot) : '.csv'
    return `${baseName}_anonymized${ext}`
  }

  // Methods
  async function loadHeaders(): Promise<void> {
    if (!selectedFile.value) {
      error.value = 'No file selected'
      return
    }

    isLoadingHeaders.value = true
    error.value = null

    const response = await getHeaders({ filePath: selectedFile.value })

    if (isApiError(response)) {
      error.value = getErrorMessage(response)
      columns.value = []
      selectedColumns.value = []
      rowCount.value = 0
    } else {
      columns.value = response.data.columns
      rowCount.value = response.data.rowCount
      // Auto-select high and medium risk columns (only those with data)
      selectedColumns.value = response.data.columns
        .filter((col) => col.sampleValues.length > 0 && (col.piiRisk === 'high' || col.piiRisk === 'medium'))
        .map((col) => col.index)
      // Set default output path
      config.value.outputPath = generateDefaultOutputPath(selectedFile.value)
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

    // Generate a seed for this preview session (reused in anonymize)
    previewSeed.value = config.value.deterministic && config.value.seed
      ? config.value.seed
      : generateSeed()

    const response = await getPreview({
      filePath: selectedFile.value,
      columns: selectedColumns.value,
      deterministic: true,
      seed: previewSeed.value,
      sampleCount: 5,
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

    // Use the preview seed if available, otherwise generate new one
    const seedToUse = previewSeed.value || generateSeed()

    const response = await anonymizeFile({
      filePath: selectedFile.value,
      outputPath: config.value.outputPath,
      columns: selectedColumns.value,
      deterministic: true,
      seed: seedToUse,
      force: true,
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
    config.value = {
      outputPath: '',
      deterministic: false,
      seed: '',
    }
    previews.value = []
    progress.value = 0
    result.value = null
    error.value = null
    previewSeed.value = ''
  }

  function clearError(): void {
    error.value = null
  }

  function setFile(filePath: string | null): void {
    if (filePath !== selectedFile.value) {
      // Reset state when file changes
      columns.value = []
      selectedColumns.value = []
      rowCount.value = 0
      previews.value = []
      result.value = null
      previewSeed.value = ''
      error.value = null
      selectedFile.value = filePath
      if (filePath) {
        config.value.outputPath = generateDefaultOutputPath(filePath)
      }
    }
  }

  return {
    // State
    selectedFile,
    columns,
    selectedColumns,
    rowCount,
    config,
    previews,
    progress,
    result,
    error,

    // Loading states
    isLoadingHeaders,
    isLoadingPreview,
    isAnonymizing,

    // Computed
    hasFile,
    hasColumns,
    hasSelectedColumns,
    canPreview,
    canAnonymize,
    isLoading,
    hasResult,

    // Methods
    loadHeaders,
    generatePreview,
    runAnonymize,
    reset,
    clearError,
    setFile,
  }
}
