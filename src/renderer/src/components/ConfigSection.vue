<script setup lang="ts">
import { ref } from 'vue'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { Button } from '@/components/ui/button'
import { ChevronDown, FolderOpen } from '@lucide/vue'
import { getErrorMessage, isApiError, selectOutputFile } from '@/lib/api'

interface Props {
  outputPath: string
  deterministic: boolean
  seed: string | null
  overwriteOutput: boolean
  disabled?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  disabled: false,
})

const emit = defineEmits<{
  'update:outputPath': [value: string]
  'update:deterministic': [value: boolean]
  'update:seed': [value: string]
  'update:overwriteOutput': [value: boolean]
  error: [message: string]
}>()

const isAdvancedOpen = ref(false)

function handleOutputPathChange(value: string | number): void {
  emit('update:outputPath', String(value))
}

async function handleOutputBrowse(): Promise<void> {
  const response = await selectOutputFile({ defaultPath: props.outputPath || undefined })

  if (isApiError(response)) {
    emit('error', getErrorMessage(response))
    return
  }

  if (response.data.filePath) {
    emit('update:outputPath', response.data.filePath)
  }
}

function handleDeterministicChange(checked: boolean): void {
  emit('update:deterministic', checked)
}

function handleSeedChange(value: string | number): void {
  emit('update:seed', String(value))
}

function handleOverwriteChange(checked: boolean): void {
  emit('update:overwriteOutput', checked)
}
</script>

<template>
  <div class="space-y-4">
    <div class="space-y-2">
      <Label for="output-path">Output Path</Label>
      <div class="flex items-center gap-2">
        <Input
          id="output-path"
          type="text"
          :model-value="outputPath"
          :disabled="disabled"
          placeholder="e.g., data_anonymized.csv"
          class="flex-1"
          @update:model-value="handleOutputPathChange"
          aria-describedby="output-path-description"
        />
        <Button
          variant="outline"
          :disabled="disabled"
          @click="handleOutputBrowse"
          aria-label="Choose output CSV file"
        >
          <FolderOpen class="mr-2 h-4 w-4" />
          Browse
        </Button>
      </div>
      <p id="output-path-description" class="text-sm text-muted-foreground">
        The path where the anonymized file will be saved
      </p>
    </div>

    <Collapsible v-model:open="isAdvancedOpen" class="space-y-2">
      <CollapsibleTrigger as-child>
        <Button
          variant="ghost"
          class="flex w-full items-center justify-between p-2"
          :disabled="disabled"
        >
          <span class="text-sm font-medium">App Settings</span>
          <ChevronDown
            class="h-4 w-4 transition-transform duration-200"
            :class="{ 'rotate-180': isAdvancedOpen }"
          />
        </Button>
      </CollapsibleTrigger>

      <CollapsibleContent class="space-y-4 rounded-lg border p-4">
        <div class="flex items-center space-x-4">
          <Switch
            id="deterministic-mode"
            :checked="deterministic"
            :disabled="disabled"
            @update:checked="handleDeterministicChange"
          />
          <div class="space-y-1">
            <Label for="deterministic-mode" class="cursor-pointer">
              Deterministic Mode
            </Label>
            <p class="text-sm text-muted-foreground">
              The same input value produces the same anonymized output.
            </p>
          </div>
        </div>

        <div class="space-y-2" :class="{ 'opacity-50': !deterministic }">
          <Label for="seed-input">Seed</Label>
          <Input
            id="seed-input"
            type="text"
            :model-value="seed ?? ''"
            :disabled="disabled || !deterministic"
            placeholder="Enter seed for reproducible results"
            @update:model-value="handleSeedChange"
            aria-describedby="seed-description"
          />
          <p id="seed-description" class="text-sm text-muted-foreground">
            Use the same seed to repeat anonymization across sessions.
          </p>
        </div>

        <div class="flex items-center space-x-4">
          <Switch
            id="overwrite-output"
            :checked="overwriteOutput"
            :disabled="disabled"
            @update:checked="handleOverwriteChange"
          />
          <div class="space-y-1">
            <Label for="overwrite-output" class="cursor-pointer">
              Overwrite Output
            </Label>
            <p class="text-sm text-muted-foreground">
              Replace the output file when it already exists.
            </p>
          </div>
        </div>
      </CollapsibleContent>
    </Collapsible>
  </div>
</template>
