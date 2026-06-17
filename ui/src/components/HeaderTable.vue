<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Badge } from '@/components/ui/badge'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { ChevronDown, ChevronUp } from 'lucide-vue-next'
import type { ColumnInfo } from '@/lib/api'

// Maximum rows to show before enabling "show more"
const MAX_VISIBLE_ROWS = 50

interface Props {
  columns: ColumnInfo[]
  selectedColumns: number[]
  loading?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  loading: false,
})

const emit = defineEmits<{
  'update:selectedColumns': [value: number[]]
}>()

// Track whether to show all columns or just the first batch
const showAll = ref(false)

// Reset showAll when columns change
watch(() => props.columns.length, () => {
  showAll.value = false
})

const selectedSet = computed(() => new Set(props.selectedColumns))

// Helper to check if a column is selectable (has data)
function isSelectableColumn(column: ColumnInfo): boolean {
  return column.sampleValues.length > 0
}

// Only consider selectable columns for "all selected" check
const selectableColumns = computed(() =>
  props.columns.filter(isSelectableColumn)
)

const allSelected = computed(
  () =>
    selectableColumns.value.length > 0 &&
    selectableColumns.value.every((col) => selectedSet.value.has(col.index))
)

const highRiskColumns = computed(() =>
  props.columns.filter((col) => col.piiRisk === 'high' && col.sampleValues.length > 0).map((col) => col.index)
)

// Determine which columns to display
const visibleColumns = computed(() => {
  if (showAll.value || props.columns.length <= MAX_VISIBLE_ROWS) {
    return props.columns
  }
  return props.columns.slice(0, MAX_VISIBLE_ROWS)
})

const hasMoreColumns = computed(() =>
  props.columns.length > MAX_VISIBLE_ROWS && !showAll.value
)

const hiddenCount = computed(() =>
  props.columns.length - MAX_VISIBLE_ROWS
)

const selectionCount = computed(
  () => `${props.selectedColumns.length} of ${props.columns.length} columns selected`
)

function toggleColumn(index: number): void {
  const newSelection = selectedSet.value.has(index)
    ? props.selectedColumns.filter((i) => i !== index)
    : [...props.selectedColumns, index]
  emit('update:selectedColumns', newSelection)
}

function selectAll(): void {
  emit(
    'update:selectedColumns',
    props.columns.filter(isSelectableColumn).map((col) => col.index)
  )
}

function deselectAll(): void {
  emit('update:selectedColumns', [])
}

function selectHighRisk(): void {
  // Only select high risk columns that have data
  emit(
    'update:selectedColumns',
    props.columns
      .filter((col) => col.piiRisk === 'high' && col.sampleValues.length > 0)
      .map((col) => col.index)
  )
}

function selectHighMediumRisk(): void {
  // Select high and medium risk columns that have data
  emit(
    'update:selectedColumns',
    props.columns
      .filter((col) => (col.piiRisk === 'high' || col.piiRisk === 'medium') && col.sampleValues.length > 0)
      .map((col) => col.index)
  )
}

function getRiskBadgeClass(risk: string): string {
  switch (risk) {
    case 'high':
      return 'bg-red-900/50 text-red-400 hover:bg-red-900/50'
    case 'medium':
      return 'bg-yellow-900/50 text-yellow-400 hover:bg-yellow-900/50'
    default:
      return 'bg-green-900/50 text-green-400 hover:bg-green-900/50'
  }
}
</script>

<template>
  <div class="space-y-4">
    <div class="flex flex-wrap gap-2">
      <Button
        variant="outline"
        size="sm"
        :disabled="loading || allSelected"
        @click="selectAll"
      >
        Select All
      </Button>
      <Button
        variant="outline"
        size="sm"
        :disabled="loading || selectedColumns.length === 0"
        @click="deselectAll"
      >
        Deselect All
      </Button>
      <Button
        variant="outline"
        size="sm"
        :disabled="loading || highRiskColumns.length === 0"
        @click="selectHighRisk"
      >
        Select High Risk
      </Button>
      <Button
        variant="outline"
        size="sm"
        :disabled="loading"
        @click="selectHighMediumRisk"
      >
        Select PII Risk
      </Button>
    </div>

    <div class="rounded-md border">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead class="w-12"></TableHead>
            <TableHead class="w-12">#</TableHead>
            <TableHead>Column Name</TableHead>
            <TableHead>Type</TableHead>
            <TableHead>Risk</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <template v-if="loading">
            <TableRow v-for="i in 5" :key="i">
              <TableCell>
                <div class="h-4 w-4 animate-pulse rounded bg-muted"></div>
              </TableCell>
              <TableCell>
                <div class="h-4 w-8 animate-pulse rounded bg-muted"></div>
              </TableCell>
              <TableCell>
                <div class="h-4 w-32 animate-pulse rounded bg-muted"></div>
              </TableCell>
              <TableCell>
                <div class="h-4 w-20 animate-pulse rounded bg-muted"></div>
              </TableCell>
              <TableCell>
                <div class="h-5 w-16 animate-pulse rounded bg-muted"></div>
              </TableCell>
            </TableRow>
          </template>
          <template v-else-if="columns.length === 0">
            <TableRow>
              <TableCell colspan="5" class="text-center text-muted-foreground">
                No columns to display
              </TableCell>
            </TableRow>
          </template>
          <template v-else>
            <TableRow
              v-for="column in visibleColumns"
              :key="column.index"
              :class="isSelectableColumn(column) ? 'cursor-pointer' : 'opacity-50'"
              @click="isSelectableColumn(column) && toggleColumn(column.index)"
            >
              <TableCell>
                <Checkbox
                  v-if="isSelectableColumn(column)"
                  :model-value="selectedSet.has(column.index)"
                  @update:model-value="toggleColumn(column.index)"
                  @click.stop
                  :aria-label="`Select column ${column.name}`"
                />
                <span v-else class="h-4 w-4 block" />
              </TableCell>
              <TableCell class="font-mono text-muted-foreground">
                {{ column.index }}
              </TableCell>
              <TableCell>
                <span :class="isSelectableColumn(column) ? 'font-medium' : 'line-through text-muted-foreground'">
                  {{ column.name }}
                </span>
                <span v-if="column.sampleValues.length === 0" class="ml-2 text-xs text-muted-foreground italic">
                  (no data)
                </span>
                <span v-else-if="column.piiRisk === 'low'" class="ml-2 text-xs text-muted-foreground italic">
                  (low risk - no PII)
                </span>
              </TableCell>
              <TableCell class="text-muted-foreground">
                {{ column.detectedType }}
              </TableCell>
              <TableCell>
                <Badge :class="getRiskBadgeClass(column.piiRisk)">
                  {{ column.piiRisk }}
                </Badge>
              </TableCell>
            </TableRow>
            <!-- Show more/less button for large column sets -->
            <TableRow v-if="hasMoreColumns || showAll">
              <TableCell colspan="5" class="text-center">
                <Button
                  variant="ghost"
                  size="sm"
                  @click.stop="showAll = !showAll"
                >
                  <ChevronDown v-if="!showAll" class="mr-2 h-4 w-4" />
                  <ChevronUp v-else class="mr-2 h-4 w-4" />
                  {{ showAll ? 'Show Less' : `Show ${hiddenCount} More Columns` }}
                </Button>
              </TableCell>
            </TableRow>
          </template>
        </TableBody>
      </Table>
    </div>

    <p class="text-sm text-muted-foreground">
      {{ selectionCount }}
    </p>
  </div>
</template>
