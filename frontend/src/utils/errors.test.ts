import { describe, expect, it } from 'vitest'
import { messageFrom } from './errors'

describe('messageFrom', () => {
  it('generalizes unexpected errors containing absolute paths', () => {
    const messages = [
      'Parser rejected /data/customer/acme.csv at row 2',
      'Parser rejected "/Users/Ada Lovelace/customer data.csv" at row 2',
      'Parser rejected /Users/Ada, Inc/customer.csv',
      'Parser rejected /Users/Ada (Work)/customer.csv',
      'Parser rejected / Customer Data/acme.csv',
      'Parser rejected //server/share/acme.csv',
      'Parser rejected path:/Users/Ada/customer.csv',
      'Parser rejected \\\\server\\Customer Data\\acme.csv',
      'Could not parse C:\\Customer Data\\acme.csv: invalid header',
      'Could not parse file:///Users/Ada/customer.csv',
      'Could not parse file://server/share/acme.csv',
      'Could not parse file:/Users/Ada/customer.csv',
    ]

    for (const message of messages) {
      expect(messageFrom(message)).toBe('Unexpected application error.')
    }
  })

  it('keeps expected file failures actionable without exposing their paths', () => {
    expect(messageFrom('File not found: /Users/Ada, Inc/customer.csv')).toBe(
      'The selected file could not be found. Check the path and try again.',
    )
    expect(messageFrom('Output file already exists: C:\\Customer Data\\output.csv')).toBe(
      'Output file already exists. Choose a different path or enable overwrite.',
    )
  })

  it('does not mistake URLs, ratios, or relative paths for absolute paths', () => {
    expect(messageFrom('Local AI unavailable at http://localhost:11434')).toBe(
      'Local AI unavailable at http://localhost:11434',
    )
    expect(messageFrom('Rejected 1/2 records in src/input.csv')).toBe(
      'Rejected 1/2 records in src/input.csv',
    )
  })
})
