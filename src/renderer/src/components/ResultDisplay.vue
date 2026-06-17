<script setup lang="ts">
import { computed } from 'vue'
import { Button } from '@/components/ui/button'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { CheckCircle2, FolderOpen, RefreshCcw } from '@lucide/vue'
import { getErrorMessage, isApiError, showOutputInFolder } from '@/lib/api'

interface Props {
  outputPath: string
  rowCount: number
  columnsAnonymized: number
  duration: number
}

const props = defineProps<Props>()

const emit = defineEmits<{
  reset: []
  error: [message: string]
}>()

const durationText = computed(() => {
  if (props.duration < 1000) {
    return `${props.duration}ms`
  }
  return `${(props.duration / 1000).toFixed(2)}s`
})

const statsText = computed(() => {
  const rows = props.rowCount.toLocaleString()
  const cols = props.columnsAnonymized
  const colText = cols === 1 ? 'column' : 'columns'
  return `${rows} rows processed, ${cols} ${colText} anonymized in ${durationText.value}`
})

function handleReset(): void {
  emit('reset')
}

async function handleOpenFolder(): Promise<void> {
  const response = await showOutputInFolder({ outputPath: props.outputPath })

  if (isApiError(response)) {
    emit('error', getErrorMessage(response))
  }
}
</script>

<template>
  <div class="space-y-4">
    <Alert class="border-green-500/50 bg-green-900/20">
      <CheckCircle2 class="h-5 w-5 text-green-500" />
      <AlertTitle class="text-green-400">Success!</AlertTitle>
      <AlertDescription class="space-y-2">
        <p>Your file has been successfully anonymized.</p>
        <p class="font-mono text-sm text-muted-foreground">
          {{ outputPath }}
        </p>
        <p class="text-sm text-muted-foreground">
          {{ statsText }}
        </p>
      </AlertDescription>
    </Alert>

    <div class="flex flex-wrap gap-2">
      <Button variant="outline" @click="handleOpenFolder">
        <FolderOpen class="mr-2 h-4 w-4" />
        Open Folder
      </Button>
      <Button @click="handleReset">
        <RefreshCcw class="mr-2 h-4 w-4" />
        Anonymize Another File
      </Button>
    </div>
  </div>
</template>
