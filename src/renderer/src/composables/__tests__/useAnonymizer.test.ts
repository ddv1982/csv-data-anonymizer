import { isProxy } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useAnonymizer } from '../useAnonymizer'
import { anonymizeFile, getPreview } from '@/lib/api'

vi.mock('@/lib/api', () => ({
  anonymizeFile: vi.fn(),
  getErrorMessage: vi.fn((error: { error: { message: string } }) => error.error.message),
  getHeaders: vi.fn(),
  getPreview: vi.fn(),
  getSettings: vi.fn(),
  isApiError: (response: { success: boolean }) => response.success === false,
  updateSettings: vi.fn(),
}))

const getPreviewMock = vi.mocked(getPreview)
const anonymizeFileMock = vi.mocked(anonymizeFile)

describe('useAnonymizer', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    getPreviewMock.mockResolvedValue({
      success: true,
      data: {
        previews: [],
      },
    })
    anonymizeFileMock.mockResolvedValue({
      success: true,
      data: {
        outputPath: '/tmp/output.csv',
        rowCount: 2,
        columnsAnonymized: 1,
        duration: 10,
      },
    })
  })

  it('sends a plain cloned column array when generating a preview', async () => {
    const anonymizer = useAnonymizer()
    anonymizer.selectedFile.value = '/tmp/input.csv'
    anonymizer.selectedColumns.value = [1]

    expect(isProxy(anonymizer.selectedColumns.value)).toBe(true)

    await anonymizer.generatePreview()

    const params = getPreviewMock.mock.calls[0]?.[0]
    expect(params?.columns).toEqual([1])
    expect(params?.columns).not.toBe(anonymizer.selectedColumns.value)
    expect(isProxy(params?.columns)).toBe(false)
  })

  it('sends a plain cloned column array when anonymizing a file', async () => {
    const anonymizer = useAnonymizer()
    anonymizer.selectedFile.value = '/tmp/input.csv'
    anonymizer.selectedColumns.value = [1]
    anonymizer.config.value.outputPath = '/tmp/output.csv'

    expect(isProxy(anonymizer.selectedColumns.value)).toBe(true)

    await anonymizer.runAnonymize()

    const params = anonymizeFileMock.mock.calls[0]?.[0]
    expect(params?.columns).toEqual([1])
    expect(params?.columns).not.toBe(anonymizer.selectedColumns.value)
    expect(isProxy(params?.columns)).toBe(false)
  })
})
