import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import ProgressBar from '../ProgressBar.vue'

describe('ProgressBar', () => {
  it('displays progress percentage', () => {
    const wrapper = mount(ProgressBar, {
      props: {
        progress: 50,
        rowsProcessed: 500,
        totalRows: 1000,
      },
    })

    expect(wrapper.text()).toContain('50%')
  })

  it('displays row count', () => {
    const wrapper = mount(ProgressBar, {
      props: {
        progress: 50,
        rowsProcessed: 500,
        totalRows: 1000,
      },
    })

    expect(wrapper.text()).toContain('500')
    // Check for total rows (locale may format differently)
    expect(wrapper.text()).toMatch(/1[,.]?000/)
  })

  it('shows cancel button when canCancel is true', () => {
    const wrapper = mount(ProgressBar, {
      props: {
        progress: 50,
        rowsProcessed: 500,
        totalRows: 1000,
        canCancel: true,
      },
    })

    const cancelButton = wrapper.find('button[aria-label="Cancel anonymization"]')
    expect(cancelButton.exists()).toBe(true)
  })

  it('hides cancel button when canCancel is false', () => {
    const wrapper = mount(ProgressBar, {
      props: {
        progress: 50,
        rowsProcessed: 500,
        totalRows: 1000,
        canCancel: false,
      },
    })

    const cancelButton = wrapper.find('button[aria-label="Cancel anonymization"]')
    expect(cancelButton.exists()).toBe(false)
  })

  it('disables cancel button when progress >= 95%', () => {
    const wrapper = mount(ProgressBar, {
      props: {
        progress: 95,
        rowsProcessed: 950,
        totalRows: 1000,
        canCancel: true,
      },
    })

    const cancelButton = wrapper.find('button[aria-label="Cancel anonymization"]')
    expect(cancelButton.attributes('disabled')).toBeDefined()
  })

  it('emits cancel event when cancel button clicked', async () => {
    const wrapper = mount(ProgressBar, {
      props: {
        progress: 50,
        rowsProcessed: 500,
        totalRows: 1000,
        canCancel: true,
      },
    })

    const cancelButton = wrapper.find('button[aria-label="Cancel anonymization"]')
    await cancelButton.trigger('click')

    expect(wrapper.emitted('cancel')).toBeTruthy()
  })

  it('shows processing message when totalRows is 0', () => {
    const wrapper = mount(ProgressBar, {
      props: {
        progress: 50,
        rowsProcessed: 0,
        totalRows: 0,
      },
    })

    expect(wrapper.text()).toContain('Processing...')
  })
})
