import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { columnMetadataFixture } from '../test-utils/builders'
import { ColumnTable } from './ColumnTable'

describe('ColumnTable', () => {
  it('allows every rendered column to be selected', async () => {
    const user = userEvent.setup()
    const onToggleColumn = vi.fn()

    render(
      <ColumnTable
        columns={[columnMetadataFixture({ name: 'email' })]}
        allColumnCount={1}
        selectedSet={new Set()}
        loading={false}
        showAllColumns={false}
        hiddenColumnCount={0}
        onToggleColumn={onToggleColumn}
        controls={{}}
        onStrategyChange={vi.fn()}
        onToggleShowAll={vi.fn()}
      />,
    )

    await user.click(screen.getByRole('checkbox', { name: /select column email/i }))

    expect(onToggleColumn).toHaveBeenCalledWith(expect.objectContaining({ name: 'email' }))
  })

  it('blocks selection and strategy controls when disabled', async () => {
    const user = userEvent.setup()
    const onToggleColumn = vi.fn()
    const onStrategyChange = vi.fn()

    render(
      <ColumnTable
        columns={[columnMetadataFixture({ name: 'email' })]}
        allColumnCount={1}
        selectedSet={new Set()}
        loading={false}
        disabled
        showAllColumns={false}
        hiddenColumnCount={0}
        onToggleColumn={onToggleColumn}
        controls={{}}
        onStrategyChange={onStrategyChange}
        onToggleShowAll={vi.fn()}
      />,
    )

    const checkbox = screen.getByRole('checkbox', { name: /select column email/i })
    const strategy = screen.getByRole('combobox', { name: /strategy for email/i })
    expect(checkbox).toBeDisabled()
    expect(strategy).toBeDisabled()

    await user.click(checkbox)
    await user.click(strategy)

    expect(onToggleColumn).not.toHaveBeenCalled()
    expect(onStrategyChange).not.toHaveBeenCalled()
  })

  it('marks columns that need detector review', () => {
    render(
      <ColumnTable
        columns={[columnMetadataFixture({ reviewReasons: ['detectorsDisagree'] })]}
        allColumnCount={1}
        selectedSet={new Set()}
        loading={false}
        showAllColumns={false}
        hiddenColumnCount={0}
        onToggleColumn={vi.fn()}
        controls={{}}
        onStrategyChange={vi.fn()}
        onToggleShowAll={vi.fn()}
      />,
    )

    expect(screen.getByText('Review')).toBeInTheDocument()
  })

  it.each([
    ['resolved', 'Resolved'],
    ['uncertain', 'Uncertain'],
    ['conflicting', 'Conflicting'],
  ] as const)('shows the backend %s semantic decision accessibly', (status, label) => {
    render(
      <ColumnTable
        columns={[columnMetadataFixture({
          name: 'entity_id',
          strategy: 'redact',
          detectedType: 'uuid',
          evidenceProfile: {
            formatEvidence: {
              dataType: 'uuid',
              confidence: 'high',
              matchCount: 4,
              sampleCount: 5,
              basis: 'detectionSample',
              detectors: ['pattern:uuid'],
            },
            semanticDecision: {
              kind: 'recordIdentifier',
              confidence: 'medium',
              status,
              specificity: 'generic',
              supportingEvidence: ['UUID format'],
              conflictingEvidence: status === 'conflicting' ? ['Header suggests a device'] : [],
              reason: 'The specific subject is unknown.',
            },
            privacyDecision: {
              risk: 'medium',
              recommendedStrategy: 'redact',
              autoSelected: true,
              reason: 'Persistent identifiers can link records.',
            },
            redactionDecision: {
              placeholder: '[ENTITY_ID]',
              source: 'columnHeader',
              isTyped: false,
              preservesEquality: false,
              reason: 'Use a non-linkable column marker.',
            },
          },
        })]}
        allColumnCount={1}
        selectedSet={new Set([0])}
        loading={false}
        showAllColumns={false}
        hiddenColumnCount={0}
        onToggleColumn={vi.fn()}
        controls={{}}
        onStrategyChange={vi.fn()}
        onToggleShowAll={vi.fn()}
      />,
    )

    expect(screen.getByLabelText(`Semantic decision: ${label}`)).toBeInTheDocument()
    expect(screen.getByText('Persistent identifier')).toBeInTheDocument()
    expect(screen.getByText('Coverage: 4 of 5 samples')).toBeInTheDocument()
    expect(screen.getByText('[ENTITY_ID]')).toBeInTheDocument()
  })

  it('keeps the authoritative decision and complete evidence details inspectable', async () => {
    const user = userEvent.setup()
    render(
      <ColumnTable
        columns={[columnMetadataFixture({
          name: 'secret_device_id',
          privacyEvidence: [{
            kind: 'credentialOrSecret',
            dataType: 'string',
            confidence: 'medium',
            matchCount: 3,
            sampleCount: 3,
            score: 82,
            detector: 'header:taxonomy:credential.secret',
            detectors: ['header:taxonomy:credential.secret'],
            reason: 'The header identifies credential or secret data.',
          }],
          evidenceProfile: {
            ...columnMetadataFixture().evidenceProfile,
            semanticDecision: {
              kind: 'credentialOrSecret',
              confidence: 'medium',
              status: 'resolved',
              specificity: 'specific',
              supportingEvidence: ['header:taxonomy:credential.secret'],
              conflictingEvidence: ['header:taxonomy:device.identifier'],
              reason: 'Credential evidence outranks device evidence.',
            },
            redactionDecision: {
              placeholder: '[SECRET]',
              source: 'typed',
              isTyped: true,
              preservesEquality: false,
              reason: 'Credential evidence supports a typed marker.',
            },
          },
        })]}
        allColumnCount={1}
        selectedSet={new Set([0])}
        loading={false}
        showAllColumns={false}
        hiddenColumnCount={0}
        onToggleColumn={vi.fn()}
        controls={{}}
        onStrategyChange={vi.fn()}
        onToggleShowAll={vi.fn()}
      />,
    )

    await user.click(screen.getByRole('button', { name: 'Explain column decision for secret_device_id' }))

    expect(screen.getByText('Supporting sources:')).toBeInTheDocument()
    expect(screen.getByText('Conflicting sources:')).toBeInTheDocument()
    expect(screen.getByText('Evidence details:')).toBeInTheDocument()
    expect(screen.getByText('3 of 3 samples', { exact: false })).toBeInTheDocument()
    expect(screen.getByText('The header identifies credential or secret data.')).toBeInTheDocument()
  })
})
