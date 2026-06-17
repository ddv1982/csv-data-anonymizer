import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import FileSelector from '../FileSelector.vue'

describe('FileSelector', () => {
  it('renders with placeholder when no file selected', () => {
    const wrapper = mount(FileSelector, {
      props: {
        modelValue: null,
      },
    })

    const input = wrapper.find('input[type="text"]')
    expect(input.attributes('placeholder')).toBe('Select a CSV file...')
  })

  it('renders selected file path', () => {
    const wrapper = mount(FileSelector, {
      props: {
        modelValue: 'test.csv',
      },
    })

    const input = wrapper.find('input[type="text"]')
    expect((input.element as HTMLInputElement).value).toBe('test.csv')
  })

  it('shows clear button when file is selected', () => {
    const wrapper = mount(FileSelector, {
      props: {
        modelValue: 'test.csv',
      },
    })

    const clearButton = wrapper.findAll('button').find(
      (btn) => btn.attributes('aria-label') === 'Clear file selection'
    )
    expect(clearButton?.exists()).toBe(true)
  })

  it('hides clear button when no file selected', () => {
    const wrapper = mount(FileSelector, {
      props: {
        modelValue: null,
      },
    })

    const clearButton = wrapper.findAll('button').find(
      (btn) => btn.attributes('aria-label') === 'Clear file selection'
    )
    expect(clearButton).toBeUndefined()
  })

  it('emits update:modelValue with null when clear button clicked', async () => {
    const wrapper = mount(FileSelector, {
      props: {
        modelValue: 'test.csv',
      },
    })

    const clearButton = wrapper.findAll('button').find(
      (btn) => btn.attributes('aria-label') === 'Clear file selection'
    )
    await clearButton?.trigger('click')

    expect(wrapper.emitted('update:modelValue')).toBeTruthy()
    expect(wrapper.emitted('update:modelValue')![0]).toEqual([null])
  })

  it('disables buttons when disabled prop is true', () => {
    const wrapper = mount(FileSelector, {
      props: {
        modelValue: 'test.csv',
        disabled: true,
      },
    })

    const buttons = wrapper.findAll('button')
    buttons.forEach((btn) => {
      expect(btn.attributes('disabled')).toBeDefined()
    })
  })

  it('has accessible browse button', () => {
    const wrapper = mount(FileSelector, {
      props: {
        modelValue: null,
      },
    })

    const browseButton = wrapper.find('button[aria-label="Browse for CSV file"]')
    expect(browseButton.exists()).toBe(true)
  })
})
