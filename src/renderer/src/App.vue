<script setup lang="ts">
import { onMounted, watch } from 'vue'
import { useAnonymizer } from '@/composables/useAnonymizer'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { AlertCircle, Loader2, Shield } from '@lucide/vue'
import FileSelector from '@/components/FileSelector.vue'
import HeaderTable from '@/components/HeaderTable.vue'
import ConfigSection from '@/components/ConfigSection.vue'
import PreviewTable from '@/components/PreviewTable.vue'
import ProgressBar from '@/components/ProgressBar.vue'
import ResultDisplay from '@/components/ResultDisplay.vue'

const {
  selectedFile,
  columns,
  selectedColumns,
  rowCount,
  config,
  previews,
  progress,
  result,
  error,
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
} = useAnonymizer()

onMounted(() => {
  loadSettings()
})

// Load headers when file is selected
watch(selectedFile, (newFile) => {
  if (newFile) {
    loadHeaders()
  }
})

function handleFileSelect(path: string | null): void {
  setFile(path)
}

function handlePreview(): void {
  generatePreview()
}

function handleAnonymize(): void {
  runAnonymize()
}

function handleReset(): void {
  reset()
}

function handleDismissError(): void {
  clearError()
}

function handleError(message: string): void {
  setError(message)
}
</script>

<template>
  <div class="min-h-screen bg-background">
    <!-- Header -->
    <header class="border-b bg-card">
      <div class="container mx-auto max-w-4xl px-4 py-4">
        <div class="flex items-center gap-3">
          <Shield class="h-8 w-8 text-primary" />
          <h1 class="text-2xl font-bold text-foreground">CSV Anonymizer</h1>
        </div>
      </div>
    </header>

    <!-- Main Content -->
    <main class="container mx-auto max-w-4xl px-4 py-8">
      <div class="space-y-6">
        <!-- Error Alert -->
        <Alert v-if="error" variant="destructive" role="alert" aria-live="polite">
          <AlertCircle class="h-4 w-4" aria-hidden="true" />
          <AlertDescription class="flex items-center justify-between">
            <span>{{ error }}</span>
            <Button variant="ghost" size="sm" @click="handleDismissError">
              Dismiss
            </Button>
          </AlertDescription>
        </Alert>

        <!-- Result Display -->
        <template v-if="hasResult && result">
                  <ResultDisplay
                    :output-path="result.outputPath"
                    :row-count="result.rowCount"
                    :columns-anonymized="result.columnsAnonymized"
                    :duration="result.duration"
                    @reset="handleReset"
                    @error="handleError"
                  />
        </template>

        <!-- Main Workflow -->
        <template v-else>
          <!-- Section 1: File Selection -->
          <Card>
            <CardHeader>
              <CardTitle class="text-lg">1. Select File</CardTitle>
            </CardHeader>
            <CardContent>
                <FileSelector
                  :model-value="selectedFile"
                  :disabled="isLoading"
                  @update:model-value="handleFileSelect"
                  @error="handleError"
                />
            </CardContent>
          </Card>

          <!-- Section 2: Column Selection -->
          <Card :class="{ 'opacity-50 pointer-events-none': !hasFile }">
            <CardHeader>
              <CardTitle class="text-lg">2. Select Columns</CardTitle>
            </CardHeader>
            <CardContent>
              <HeaderTable
                :columns="columns"
                :selected-columns="selectedColumns"
                :loading="isLoadingHeaders"
                @update:selected-columns="(val) => (selectedColumns = val)"
              />
            </CardContent>
          </Card>

          <!-- Section 3: Configuration -->
          <Card :class="{ 'opacity-50 pointer-events-none': !hasColumns }">
            <CardHeader>
              <CardTitle class="text-lg">3. Configuration</CardTitle>
            </CardHeader>
            <CardContent>
                <ConfigSection
                  :output-path="config.outputPath"
                  :deterministic="config.deterministic"
                  :seed="config.seed"
                  :overwrite-output="config.overwriteOutput"
                  :disabled="!hasColumns || isLoading"
                  @update:output-path="setOutputPath"
                  @update:deterministic="setDeterministic"
                  @update:seed="setSeed"
                  @update:overwrite-output="setOverwriteOutput"
                  @error="handleError"
                />
            </CardContent>
          </Card>

          <!-- Section 4: Preview -->
          <Card :class="{ 'opacity-50 pointer-events-none': !hasSelectedColumns }">
            <CardHeader class="flex flex-row items-center justify-between">
              <CardTitle class="text-lg">4. Preview (Optional)</CardTitle>
              <Button
                variant="outline"
                size="sm"
                :disabled="!canPreview"
                @click="handlePreview"
              >
                <Loader2 v-if="isLoadingPreview" class="mr-2 h-4 w-4 animate-spin" />
                Show Preview
              </Button>
            </CardHeader>
            <CardContent>
              <PreviewTable :previews="previews" :loading="isLoadingPreview" />
            </CardContent>
          </Card>

          <!-- Anonymize Button / Progress -->
          <Card>
            <CardContent class="pt-6">
              <template v-if="isAnonymizing">
                <ProgressBar
                  :progress="progress"
                  :rows-processed="Math.round((progress / 100) * rowCount)"
                  :total-rows="rowCount"
                />
              </template>
              <template v-else>
                <Button
                  class="w-full"
                  size="lg"
                  :disabled="!canAnonymize || isLoading"
                  @click="handleAnonymize"
                >
                  <Loader2 v-if="isLoading" class="mr-2 h-5 w-5 animate-spin" />
                  <Shield v-else class="mr-2 h-5 w-5" />
                  {{ isLoading ? 'Processing...' : 'Anonymize File' }}
                </Button>
              </template>
            </CardContent>
          </Card>
        </template>
      </div>
    </main>

    <!-- Footer -->
    <footer class="border-t bg-card mt-auto">
      <div class="container mx-auto max-w-4xl px-4 py-4">
        <p class="text-center text-sm text-muted-foreground">
          CSV Anonymizer - Protect sensitive data in your CSV files
        </p>
      </div>
    </footer>
  </div>
</template>
