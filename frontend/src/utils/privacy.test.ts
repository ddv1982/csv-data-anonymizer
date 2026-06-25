import { describe, expect, it } from 'vitest'
import { defaultSettings, privacyConfigFromSettings } from '../defaults'
import type { HeadersData, PrivacyConfig } from '../types'
import { getPrivacyConfigValidation, getPrivacyScaleWarning } from './privacy'

describe('getPrivacyConfigValidation', () => {
  it('accepts standard mode without extra release settings', () => {
    expect(getPrivacyConfigValidation(defaultPrivacyConfig()).valid).toBe(true)
  })

  it('validates formal privacy thresholds and sensitive-column requirements', () => {
    expect(validate({ releaseMode: 'formalTabular', formal: { ...defaultPrivacyConfig().formal, k: 0 } }).reason)
      .toMatch(/Set k/)
    expect(
      validate({
        releaseMode: 'formalTabular',
        formal: { ...defaultPrivacyConfig().formal, lDiversity: 0 },
      }).reason,
    ).toMatch(/l-diversity/)
    expect(
      validate({
        releaseMode: 'formalTabular',
        formal: { ...defaultPrivacyConfig().formal, tCloseness: 1.5 },
      }).reason,
    ).toMatch(/t-closeness/)
    expect(
      getPrivacyConfigValidation(
        {
          ...defaultPrivacyConfig(),
          releaseMode: 'formalTabular',
          formal: { ...defaultPrivacyConfig().formal, lDiversity: 2 },
          columnRoles: [{ columnIndex: 1, role: 'sensitive', generalizationLevel: 0 }],
        },
        new Set([0]),
      ).reason,
    ).toMatch(/Sensitive/)
  })

  it('validates differential privacy aggregate settings', () => {
    expect(
      validate({
        releaseMode: 'differentialPrivacyAggregate',
        differentialPrivacy: { ...defaultPrivacyConfig().differentialPrivacy, epsilon: 0 },
      }).reason,
    ).toMatch(/epsilon/)
    expect(
      validate({
        releaseMode: 'differentialPrivacyAggregate',
        differentialPrivacy: {
          ...defaultPrivacyConfig().differentialPrivacy,
          publicGroupValues: ['A'],
        },
      }).reason,
    ).toMatch(/Clear allowed group values/)
    expect(
      validate({
        releaseMode: 'differentialPrivacyAggregate',
        differentialPrivacy: {
          ...defaultPrivacyConfig().differentialPrivacy,
          aggregate: 'count',
          valueColumn: 1,
        },
      }).reason,
    ).toMatch(/Clear the value column/)
    expect(
      validate({
        releaseMode: 'differentialPrivacyAggregate',
        differentialPrivacy: {
          ...defaultPrivacyConfig().differentialPrivacy,
          privacyUnitColumn: 0,
          maxContributionsPerUnit: 0,
        },
      }).reason,
    ).toMatch(/max contributions/)
  })

  it('requires selected and public grouped DP columns', () => {
    const config = {
      ...defaultPrivacyConfig(),
      releaseMode: 'differentialPrivacyAggregate',
      columnRoles: [{ columnIndex: 1, role: 'attribute', generalizationLevel: 0 }],
      differentialPrivacy: {
        ...defaultPrivacyConfig().differentialPrivacy,
        groupByColumn: 1,
        groupLabelsPublic: false,
        publicGroupValues: ['A'],
      },
    } satisfies PrivacyConfig

    expect(getPrivacyConfigValidation(config, new Set([0]), 2).reason).toMatch(/Select the DP group column/)
    expect(getPrivacyConfigValidation(config, new Set([0, 1]), 2).reason).toMatch(/public/)
    expect(
      getPrivacyConfigValidation(
        {
          ...config,
          differentialPrivacy: {
            ...config.differentialPrivacy,
            groupLabelsPublic: true,
            publicGroupValues: [''],
          },
        },
        new Set([0, 1]),
        2,
      ).reason,
    ).toMatch(/allowed group values|blank/)
  })

  it('validates DP sum and mean value columns and public bounds', () => {
    expect(
      validate({
        releaseMode: 'differentialPrivacyAggregate',
        differentialPrivacy: {
          ...defaultPrivacyConfig().differentialPrivacy,
          aggregate: 'sum',
        },
      }).reason,
    ).toMatch(/numeric value column/)
    expect(
      validate({
        releaseMode: 'differentialPrivacyAggregate',
        differentialPrivacy: {
          ...defaultPrivacyConfig().differentialPrivacy,
          aggregate: 'mean',
          valueColumn: 1,
        },
      }).reason,
    ).toMatch(/lower and upper bounds/)
    expect(
      validate({
        releaseMode: 'differentialPrivacyAggregate',
        differentialPrivacy: {
          ...defaultPrivacyConfig().differentialPrivacy,
          aggregate: 'mean',
          valueColumn: 1,
          lowerBound: 10,
          upperBound: 1,
        },
      }).reason,
    ).toMatch(/less than or equal/)
  })

  it('validates DP budget tracking settings', () => {
    expect(
      validate({
        releaseMode: 'differentialPrivacyAggregate',
        differentialPrivacy: {
          ...defaultPrivacyConfig().differentialPrivacy,
          budget: {
            ...defaultPrivacyConfig().differentialPrivacy.budget,
            enabled: true,
            limitEpsilon: null,
          },
        },
      }).reason,
    ).toMatch(/budget limit/)
  })

  it('keeps synthetic data language conservative', () => {
    expect(
      validate({
        releaseMode: 'syntheticData',
        synthetic: { ...defaultPrivacyConfig().synthetic, epsilon: 1 },
      }).reason,
    ).toMatch(/does not provide a DP synthetic guarantee/)
    expect(getPrivacyConfigValidation({ ...defaultPrivacyConfig(), releaseMode: 'syntheticData' }, new Set([0]), 2).reason)
      .toMatch(/Select every CSV column/)
  })
})

describe('getPrivacyScaleWarning', () => {
  it('does not warn for standard row-level transformation', () => {
    expect(getPrivacyScaleWarning(defaultPrivacyConfig(), headersFixture({ rowCount: 200_000 }))).toBeNull()
  })

  it('warns when full-dataset privacy modes are large', () => {
    const warning = getPrivacyScaleWarning(
      { ...defaultPrivacyConfig(), releaseMode: 'formalTabular' },
      headersFixture({ rowCount: 200_000 }),
    )

    expect(warning).toContain('k/l/t tabular output')
    expect(warning).toContain('200,000')
    expect(warning).toContain('1,000,000')
  })

  it('warns when exact row count is not known yet for full-dataset modes', () => {
    const warning = getPrivacyScaleWarning(
      { ...defaultPrivacyConfig(), releaseMode: 'differentialPrivacyAggregate' },
      headersFixture({ rowCount: 100, rowCountIsComplete: false }),
    )

    expect(warning).toContain('DP aggregate output')
    expect(warning).toContain('Exact row count is still being calculated')
  })
})

function validate(config: Partial<PrivacyConfig>) {
  return getPrivacyConfigValidation({ ...defaultPrivacyConfig(), ...config })
}

function defaultPrivacyConfig() {
  return privacyConfigFromSettings(defaultSettings)
}

function headersFixture(overrides: Partial<Pick<HeadersData, 'rowCount' | 'rowCountIsComplete'>> = {}): HeadersData {
  return {
    filePath: '/data/input.csv',
    rowCount: overrides.rowCount ?? 2,
    rowCountIsComplete: overrides.rowCountIsComplete ?? true,
    defaultOutputPath: '/data/input_private_output.csv',
    columns: [],
  }
}
