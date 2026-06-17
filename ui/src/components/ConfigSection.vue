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
import { ChevronDown } from 'lucide-vue-next'

interface Props {
  outputPath: string
  deterministic: boolean
  seed: string | null
  disabled?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  disabled: false,
})

const emit = defineEmits<{
  'update:outputPath': [value: string]
  'update:deterministic': [value: boolean]
  'update:seed': [value: string]
}>()

const isAdvancedOpen = ref(false)

function handleOutputPathChange(event: Event): void {
  const input = event.target as HTMLInputElement
  emit('update:outputPath', input.value)
}

function handleDeterministicChange(checked: boolean): void {
  emit('update:deterministic', checked)
}

function handleSeedChange(event: Event): void {
  const input = event.target as HTMLInputElement
  emit('update:seed', input.value)
}
</script>

<template>
  <div class="space-y-4">
    <div class="space-y-2">
      <Label for="output-path">Output Path</Label>
      <Input
        id="output-path"
        type="text"
        :value="outputPath"
        :disabled="disabled"
        placeholder="e.g., data_anonymized.csv"
        @input="handleOutputPathChange"
        aria-describedby="output-path-description"
      />
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
          <span class="text-sm font-medium">Advanced Options</span>
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
              When enabled, the same input value always produces the same anonymized
              output. Useful for maintaining referential integrity between files.
            </p>
          </div>
        </div>

        <div class="space-y-2" :class="{ 'opacity-50': !deterministic }">
          <Label for="seed-input">Seed</Label>
          <Input
            id="seed-input"
            type="text"
            :value="seed ?? ''"
            :disabled="disabled || !deterministic"
            placeholder="Enter seed for reproducible results"
            @input="handleSeedChange"
            aria-describedby="seed-description"
          />
          <p id="seed-description" class="text-sm text-muted-foreground">
            A seed value ensures reproducible results. Use the same seed to get
            identical anonymization across different sessions.
          </p>
        </div>
      </CollapsibleContent>
    </Collapsible>
  </div>
</template>
