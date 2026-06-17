import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import PreviewTable from '../PreviewTable.vue'
import type { ColumnPreview } from '@/lib/api'

const mockPreviews: ColumnPreview[] = [
  {
    columnIndex: 1,
    columnName: 'email',
    samples: [
      { original: 'john@example.com', anonymized: 'anon1@example.com' },
      { original: 'jane@example.com', anonymized: 'anon2@example.com' },
    ],
  },
  {
    columnIndex: 2,
    columnName: 'phone',
    samples: [
      { original: '+1-555-1234', anonymized: '+1-555-9999' },
    ],
  },
]

describe('PreviewTable', () => {
  it('renders column names', () => {
    const wrapper = mount(PreviewTable, {
      props: {
        previews: mockPreviews,
      },
    })

    expect(wrapper.text()).toContain('email')
    expect(wrapper.text()).toContain('phone')
  })

  it('renders original and anonymized values', () => {
    const wrapper = mount(PreviewTable, {
      props: {
        previews: mockPreviews,
      },
    })

    expect(wrapper.text()).toContain('john@example.com')
    expect(wrapper.text()).toContain('anon1@example.com')
    expect(wrapper.text()).toContain('+1-555-1234')
    expect(wrapper.text()).toContain('+1-555-9999')
  })

  it('shows empty message when no previews', () => {
    const wrapper = mount(PreviewTable, {
      props: {
        previews: [],
      },
    })

    expect(wrapper.text()).toContain('No preview data available')
  })

  it('shows loading skeleton when loading', () => {
    const wrapper = mount(PreviewTable, {
      props: {
        previews: [],
        loading: true,
      },
    })

    const skeletons = wrapper.findAll('.animate-pulse')
    expect(skeletons.length).toBeGreaterThan(0)
  })

  it('displays column index in parentheses', () => {
    const wrapper = mount(PreviewTable, {
      props: {
        previews: mockPreviews,
      },
    })

    expect(wrapper.text()).toContain('(column 1)')
    expect(wrapper.text()).toContain('(column 2)')
  })
})
