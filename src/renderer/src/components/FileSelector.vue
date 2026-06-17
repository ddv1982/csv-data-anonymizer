<script setup lang="ts">
import { ref, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { X, FolderOpen } from '@lucide/vue'
import { getErrorMessage, isApiError, selectCsvFile } from '@/lib/api'

interface Props {
  modelValue: string | null
  disabled?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  disabled: false,
})

const emit = defineEmits<{
  'update:modelValue': [value: string | null]
  error: [message: string]
}>()

const manualPath = ref(props.modelValue ?? '')

watch(
  () => props.modelValue,
  (value) => {
    manualPath.value = value ?? ''
  }
)

async function handleBrowseClick(): Promise<void> {
  const response = await selectCsvFile()

  if (isApiError(response)) {
    emit('error', getErrorMessage(response))
    return
  }

  if (response.data.filePath) {
    manualPath.value = response.data.filePath
    emit('update:modelValue', response.data.filePath)
  }
}

function handlePathInput(value: string | number): void {
  const path = String(value).trim()
  manualPath.value = path

  emit('update:modelValue', path || null)
}

function handleClear(): void {
  manualPath.value = ''
  emit('update:modelValue', null)
}
</script>

<template>
  <div class="space-y-2">
    <div class="flex items-center gap-2">
      <Button
        variant="outline"
        :disabled="disabled"
        @click="handleBrowseClick"
        aria-label="Browse for CSV file"
      >
        <FolderOpen class="mr-2 h-4 w-4" />
        Browse
      </Button>

      <Input
        type="text"
        :model-value="manualPath"
        :disabled="disabled"
        placeholder="Select a CSV file..."
        class="flex-1"
        aria-label="File path input"
        @update:model-value="handlePathInput"
      />

      <Button
        v-if="modelValue"
        variant="ghost"
        size="icon"
        :disabled="disabled"
        @click="handleClear"
        aria-label="Clear file selection"
      >
        <X class="h-4 w-4" />
      </Button>
    </div>
  </div>
</template>
