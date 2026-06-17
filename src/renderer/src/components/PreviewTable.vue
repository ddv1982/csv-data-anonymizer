<script setup lang="ts">
import type { ColumnPreview } from '@/lib/api'
import { ArrowRight } from '@lucide/vue'

interface Props {
  previews: ColumnPreview[]
  loading?: boolean
}

withDefaults(defineProps<Props>(), {
  loading: false,
})
</script>

<template>
  <div class="space-y-6">
    <template v-if="loading">
      <div v-for="i in 2" :key="i" class="space-y-3">
        <div class="h-5 w-32 animate-pulse rounded bg-muted"></div>
        <div v-for="j in 3" :key="j" class="flex items-center gap-4">
          <div class="h-4 w-40 animate-pulse rounded bg-muted"></div>
          <div class="h-4 w-4 animate-pulse rounded bg-muted"></div>
          <div class="h-4 w-40 animate-pulse rounded bg-muted"></div>
        </div>
      </div>
    </template>

    <template v-else-if="previews.length === 0">
      <p class="text-center text-muted-foreground">
        No preview data available. Select columns and click "Show Preview".
      </p>
    </template>

    <template v-else>
      <div v-for="preview in previews" :key="preview.columnIndex" class="space-y-3">
        <h4 class="font-medium text-foreground">
          {{ preview.columnName }}
          <span class="text-muted-foreground">(column {{ preview.columnIndex }})</span>
        </h4>
        <div class="space-y-2 rounded-lg border p-4">
          <div
            v-if="preview.samples.length === 0"
            class="text-sm text-muted-foreground italic"
          >
            No data in sample rows for this column
          </div>
          <div
            v-else
            v-for="(sample, idx) in preview.samples"
            :key="idx"
            class="grid grid-cols-[1fr_auto_1fr] items-center gap-4"
          >
            <div class="font-mono text-sm text-muted-foreground truncate" :title="sample.original">
              {{ sample.original }}
            </div>
            <ArrowRight class="h-4 w-4 text-muted-foreground flex-shrink-0" />
            <div class="font-mono text-sm text-primary truncate" :title="sample.anonymized">
              {{ sample.anonymized }}
            </div>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
