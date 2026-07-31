import { describe, expect, it } from 'vitest'
import { columnMetadataFixture } from '../test-utils/builders'
import type { AnalyzeResponse } from '../types'
import { completeEvidenceProfile, validateAnalyzeResponse } from './analysisContract'

function responseWith(column = columnMetadataFixture()): AnalyzeResponse {
  return {
    headers: {
      filePath: '/data/input.csv',
      rowCount: 1,
      rowCountIsComplete: true,
      defaultOutputPath: '/data/input_private_output.csv',
      columns: [column],
      detectionRunSummary: {
        deterministic: 'completed',
        localNer: 'disabled',
        examinedCells: 1,
        totalEligibleCells: 1,
        skippedOversizedCells: 0,
        acceptedCandidates: 0,
        rejectedCandidates: 0,
      },
    },
    selectedColumns: [0],
    suggestedOutputPath: '/data/input_private_output.csv',
  }
}

describe('analysis response contract', () => {
  it('accepts a complete backend decision profile', () => {
    const response = responseWith()
    expect(validateAnalyzeResponse(response)).toBe(response)
    expect(completeEvidenceProfile(response.headers.columns[0].evidenceProfile)).not.toBeNull()
  })

  it('rejects a missing profile before it reaches React state', () => {
    const response = responseWith()
    Reflect.deleteProperty(response.headers.columns[0], 'evidenceProfile')

    expect(() => validateAnalyzeResponse(response)).toThrow('incompatible decision data')
  })

  it('rejects non-string evidence entries that React cannot render safely', () => {
    const response = responseWith()
    const semantic = response.headers.columns[0].evidenceProfile.semanticDecision
    semantic.supportingEvidence = [{ detector: 'invalid' } as unknown as string]

    expect(() => validateAnalyzeResponse(response)).toThrow('incompatible decision data')
  })

  it('rejects invalid numeric and enum fields instead of partially trusting the profile', () => {
    const response = responseWith()
    response.headers.columns[0].evidenceProfile.formatEvidence.matchCount = Number.NaN
    response.headers.columns[0].evidenceProfile.semanticDecision.status = 'newStatus' as 'resolved'

    expect(() => validateAnalyzeResponse(response)).toThrow('incompatible decision data')
  })
})
