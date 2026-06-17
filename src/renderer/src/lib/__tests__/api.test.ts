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
      'Output file already exists. Enable overwrite or choose a different output path.'
    )
  })

  it('surfaces unexpected bridge error details when no mapped recovery exists', () => {
    const error: ApiError = {
      success: false,
      error: {
        code: 'UNKNOWN',
        message: 'An object could not be cloned.',
      },
    }

    expect(getErrorMessage(error)).toBe('An object could not be cloned.')
  })

  it('uses a clear mapped message for bridge payload clone failures', () => {
    const error: ApiError = {
      success: false,
      error: {
        code: 'BRIDGE_PAYLOAD_INVALID',
        message: 'The app could not send the selected data to the desktop process. Try selecting the columns again.',
      },
    }

    expect(getErrorMessage(error)).toBe(
      'The app could not send the selected data to the desktop process. Try selecting the columns again.'
    )
  })
})
