import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import ResultDisplay from '../ResultDisplay.vue'

describe('ResultDisplay', () => {
  const defaultProps = {
    outputPath: '/path/to/output.csv',
    rowCount: 1000,
    columnsAnonymized: 3,
    duration: 1500,
  }

  it('displays success message', () => {
    const wrapper = mount(ResultDisplay, {
      props: defaultProps,
    })

    expect(wrapper.text()).toContain('Success!')
  })

  it('displays output path', () => {
    const wrapper = mount(ResultDisplay, {
      props: defaultProps,
    })

    expect(wrapper.text()).toContain('/path/to/output.csv')
  })

  it('displays row count', () => {
    const wrapper = mount(ResultDisplay, {
      props: defaultProps,
    })

    // Check for row count (locale may format differently)
    expect(wrapper.text()).toMatch(/1[,.]?000/)
  })

  it('displays columns anonymized count', () => {
    const wrapper = mount(ResultDisplay, {
      props: defaultProps,
    })

    expect(wrapper.text()).toContain('3')
  })

  it('displays duration in seconds', () => {
    const wrapper = mount(ResultDisplay, {
      props: defaultProps,
    })

    expect(wrapper.text()).toContain('1.50s')
  })

  it('displays duration in milliseconds when < 1000ms', () => {
    const wrapper = mount(ResultDisplay, {
      props: {
        ...defaultProps,
        duration: 500,
      },
    })

    expect(wrapper.text()).toContain('500ms')
  })

  it('has Open Folder button', () => {
    const wrapper = mount(ResultDisplay, {
      props: defaultProps,
    })

    expect(wrapper.text()).toContain('Open Folder')
  })

  it('has Anonymize Another File button', () => {
    const wrapper = mount(ResultDisplay, {
      props: defaultProps,
    })

    expect(wrapper.text()).toContain('Anonymize Another File')
  })

  it('emits reset when Anonymize Another File clicked', async () => {
    const wrapper = mount(ResultDisplay, {
      props: defaultProps,
    })

    const resetButton = wrapper.findAll('button').find(
      (btn) => btn.text().includes('Anonymize Another File')
    )
    await resetButton?.trigger('click')

    expect(wrapper.emitted('reset')).toBeTruthy()
  })

  it('uses singular "column" when only one column anonymized', () => {
    const wrapper = mount(ResultDisplay, {
      props: {
        ...defaultProps,
        columnsAnonymized: 1,
      },
    })

    expect(wrapper.text()).toContain('1 column anonymized')
  })

  it('uses plural "columns" when multiple columns anonymized', () => {
    const wrapper = mount(ResultDisplay, {
      props: defaultProps,
    })

    expect(wrapper.text()).toContain('3 columns anonymized')
  })
})
