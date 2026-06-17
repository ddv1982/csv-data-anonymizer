<script setup lang="ts">
import { ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { X, FolderOpen } from 'lucide-vue-next'

interface Props {
  modelValue: string | null
  disabled?: boolean
}

withDefaults(defineProps<Props>(), {
  disabled: false,
})

const emit = defineEmits<{
  'update:modelValue': [value: string | null]
  error: [message: string]
}>()

const fileInput = ref<HTMLInputElement | null>(null)
const manualPath = ref('')

function handleBrowseClick(): void {
  fileInput.value?.click()
}

function handleFileChange(event: Event): void {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]

  if (file) {
    // Browser security prevents access to full file path
    // Use just the filename - server resolves relative to working directory
    manualPath.value = file.name
    emit('update:modelValue', file.name)
  }

  // Reset input value to allow re-selecting the same file
  input.value = ''
}

function handlePathInput(event: Event): void {
  const input = event.target as HTMLInputElement
  const path = input.value.trim()
  manualPath.value = path

  if (path) {
    emit('update:modelValue', path)
  } else {
    emit('update:modelValue', null)
  }
}

function handleClear(): void {
  manualPath.value = ''
  emit('update:modelValue', null)
}
</script>

<template>
  <div class="space-y-2">
    <div class="flex items-center gap-2">
      <input
        ref="fileInput"
        type="file"
        accept=".csv"
        class="hidden"
        :disabled="disabled"
        @change="handleFileChange"
      />

      <Button
        variant="outline"
        :disabled="disabled"
        @click="handleBrowseClick"
        aria-label="Browse for CSV file (shows filename only)"
      >
        <FolderOpen class="mr-2 h-4 w-4" />
        Browse
      </Button>

      <Input
        type="text"
        :value="manualPath"
        :disabled="disabled"
        placeholder="Enter full file path, e.g., /home/user/data.csv"
        class="flex-1"
        aria-label="File path input"
        @input="handlePathInput"
        @blur="handlePathInput"
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
    <p class="text-xs text-muted-foreground">
      Enter the full path to your CSV file. The Browse button shows the filename for reference.
    </p>
  </div>
</template>
