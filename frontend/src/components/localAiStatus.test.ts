import { describe, expect, it } from 'vitest'
import { localAiStatus } from './localAiStatus'

describe('localAiStatus', () => {
  it.each([
    [false, false, false, false, 'Off'],
    [true, true, false, true, 'Downloading'],
    [true, false, true, true, 'Ready'],
    [true, false, false, true, 'Setup needed'],
    [true, false, false, false, 'Checking'],
  ] as const)('formats each Local AI state', (enabled, downloading, ready, hasStatus, label) => {
    expect(localAiStatus(enabled, downloading, ready, hasStatus).label).toBe(label)
  })
})
