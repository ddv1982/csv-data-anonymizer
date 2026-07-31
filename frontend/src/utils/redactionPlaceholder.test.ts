import { describe, expect, it } from 'vitest'
import { columnMetadataFixture } from '../test-utils/builders'
import { columnRedactionPlaceholder } from './redactionPlaceholder'

describe('columnRedactionPlaceholder', () => {
  it('uses the backend-issued marker without reinterpreting local evidence', () => {
    const column = columnMetadataFixture({
      detectedType: 'email',
      evidenceProfile: {
        formatEvidence: {
          dataType: 'email',
          confidence: 'high',
          matchCount: 3,
          sampleCount: 3,
          basis: 'detectionSample',
          detectors: ['pattern:email'],
        },
        semanticDecision: {
          kind: 'recordIdentifier',
          confidence: 'medium',
          status: 'uncertain',
          specificity: 'generic',
          supportingEvidence: [],
          conflictingEvidence: [],
          reason: 'The subject is unknown.',
        },
        privacyDecision: {
          risk: 'medium',
          recommendedStrategy: 'redact',
          autoSelected: true,
          reason: 'Persistent values can link records.',
        },
        redactionDecision: {
          placeholder: '[BACKEND_DECISION]',
          source: 'columnHeader',
          isTyped: false,
          preservesEquality: false,
          reason: 'Semantic evidence is not specific enough.',
        },
      },
    })

    expect(columnRedactionPlaceholder(column)).toBe('[BACKEND_DECISION]')
  })
})
