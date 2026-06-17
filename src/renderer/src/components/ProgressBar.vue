<script setup lang="ts">
import { computed } from 'vue'
import { Progress } from '@/components/ui/progress'
import { Button } from '@/components/ui/button'
import { X } from '@lucide/vue'

interface Props {
  progress: number
  rowsProcessed: number
  totalRows: number
  canCancel?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  canCancel: false,
})

const emit = defineEmits<{
  cancel: []
}>()

const progressText = computed(() => {
  if (props.totalRows === 0) {
    return 'Processing...'
  }
  return `Processing row ${props.rowsProcessed.toLocaleString()} of ${props.totalRows.toLocaleString()}`
})

const percentText = computed(() => `${Math.round(props.progress)}%`)

function handleCancel(): void {
  emit('cancel')
}
</script>

<template>
  <div class="space-y-3">
    <div class="flex items-center justify-between">
      <span class="text-sm text-muted-foreground">{{ progressText }}</span>
      <span class="text-sm font-medium">{{ percentText }}</span>
    </div>

    <Progress :model-value="progress" class="h-2" />

    <div v-if="canCancel" class="flex justify-end">
      <Button
        variant="outline"
        size="sm"
        :disabled="progress >= 95"
        @click="handleCancel"
        aria-label="Cancel anonymization"
      >
        <X class="mr-2 h-4 w-4" />
        Cancel
      </Button>
    </div>
  </div>
</template>
