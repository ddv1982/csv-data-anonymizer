import { describe, expect, it } from 'vitest'
import { getErrorMessage, type ApiError } from '../api'

describe('getErrorMessage', () => {
  it('uses backend CSV parse details instead of replacing them with a generic message', () => {
    const error: ApiError = {
      success: false,
      error: {
        code: 'CSV_PARSE_ERROR',
        message: 'CSV parse error at row 2: Missing closing quote',
        suggestion: 'Check the CSV format at row 2.',
      },
    }

    expect(getErrorMessage(error)).toBe(
      'CSV parse error at row 2: Missing closing quote Check the CSV format at row 2.'
    )
  })

  it('keeps concise mapped messages for common non-CSV errors', () => {
    const error: ApiError = {
      success: false,
      error: {
        code: 'OUTPUT_EXISTS',
        message: 'Output file already exists: /tmp/out.csv',
      },
    }

    expect(getErrorMessage(error)).toBe(
      'Output file already exists. Disable overwrite or choose a different output path.'
    )
  })
})
