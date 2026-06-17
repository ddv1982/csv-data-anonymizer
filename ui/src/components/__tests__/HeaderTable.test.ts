import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import HeaderTable from '../HeaderTable.vue'
import type { ColumnInfo } from '@/lib/api'

const mockColumns: ColumnInfo[] = [
  {
    index: 0,
    name: 'id',
    detectedType: 'numeric_id',
    confidence: 'high',
    piiRisk: 'medium',
    sampleValues: ['1', '2', '3'],
  },
  {
    index: 1,
    name: 'email',
    detectedType: 'email',
    confidence: 'high',
    piiRisk: 'high',
    sampleValues: ['test@example.com'],
  },
  {
    index: 2,
    name: 'name',
    detectedType: 'string',
    confidence: 'medium',
    piiRisk: 'low',
    sampleValues: ['John'],
  },
]

describe('HeaderTable', () => {
  it('renders column names', () => {
    const wrapper = mount(HeaderTable, {
      props: {
        columns: mockColumns,
        selectedColumns: [],
      },
    })

    expect(wrapper.text()).toContain('id')
    expect(wrapper.text()).toContain('email')
    expect(wrapper.text()).toContain('name')
  })

  it('shows selection count', () => {
    const wrapper = mount(HeaderTable, {
      props: {
        columns: mockColumns,
        selectedColumns: [0, 1],
      },
    })

    expect(wrapper.text()).toContain('2 of 3 columns selected')
  })

  it('displays risk badges with appropriate labels', () => {
    const wrapper = mount(HeaderTable, {
      props: {
        columns: mockColumns,
        selectedColumns: [],
      },
    })

    expect(wrapper.text()).toContain('high')
    expect(wrapper.text()).toContain('medium')
    expect(wrapper.text()).toContain('low')
  })

  it('shows loading skeleton when loading', () => {
    const wrapper = mount(HeaderTable, {
      props: {
        columns: [],
        selectedColumns: [],
        loading: true,
      },
    })

    const skeletons = wrapper.findAll('.animate-pulse')
    expect(skeletons.length).toBeGreaterThan(0)
  })

  it('shows empty message when no columns', () => {
    const wrapper = mount(HeaderTable, {
      props: {
        columns: [],
        selectedColumns: [],
        loading: false,
      },
    })

    expect(wrapper.text()).toContain('No columns to display')
  })

  it('emits update:selectedColumns when checkbox toggled', async () => {
    const wrapper = mount(HeaderTable, {
      props: {
        columns: mockColumns,
        selectedColumns: [],
      },
    })

    // Click on the first row
    const rows = wrapper.findAll('tr')
    // Skip header row (index 0)
    const firstDataRow = rows[1]
    expect(firstDataRow).toBeDefined()
    await firstDataRow!.trigger('click')

    expect(wrapper.emitted('update:selectedColumns')).toBeTruthy()
    expect(wrapper.emitted('update:selectedColumns')![0]).toEqual([[0]])
  })

  it('emits all columns when Select All clicked', async () => {
    const wrapper = mount(HeaderTable, {
      props: {
        columns: mockColumns,
        selectedColumns: [],
      },
    })

    const selectAllBtn = wrapper.findAll('button').find(
      (btn) => btn.text() === 'Select All'
    )
    expect(selectAllBtn).toBeDefined()
    await selectAllBtn!.trigger('click')

    expect(wrapper.emitted('update:selectedColumns')).toBeTruthy()
    expect(wrapper.emitted('update:selectedColumns')![0]).toEqual([[0, 1, 2]])
  })

  it('emits empty array when Deselect All clicked', async () => {
    const wrapper = mount(HeaderTable, {
      props: {
        columns: mockColumns,
        selectedColumns: [0, 1],
      },
    })

    const deselectAllBtn = wrapper.findAll('button').find(
      (btn) => btn.text() === 'Deselect All'
    )
    expect(deselectAllBtn).toBeDefined()
    await deselectAllBtn!.trigger('click')

    expect(wrapper.emitted('update:selectedColumns')).toBeTruthy()
    expect(wrapper.emitted('update:selectedColumns')![0]).toEqual([[]])
  })

  it('emits high risk columns when Select High Risk clicked', async () => {
    const wrapper = mount(HeaderTable, {
      props: {
        columns: mockColumns,
        selectedColumns: [],
      },
    })

    const selectHighRiskBtn = wrapper.findAll('button').find(
      (btn) => btn.text() === 'Select High Risk'
    )
    expect(selectHighRiskBtn).toBeDefined()
    await selectHighRiskBtn!.trigger('click')

    expect(wrapper.emitted('update:selectedColumns')).toBeTruthy()
    // Only column index 1 (email) has high risk
    expect(wrapper.emitted('update:selectedColumns')![0]).toEqual([[1]])
  })
})
