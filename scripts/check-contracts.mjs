#!/usr/bin/env node
import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'

const root = process.cwd()
const rustTypes = fs.readFileSync(path.join(root, 'crates/csv-anonymizer-core/src/types.rs'), 'utf8')
const tsTypes = fs.readFileSync(path.join(root, 'frontend/src/types.ts'), 'utf8')

const enumContracts = [
  'DataType',
  'Confidence',
  'PiiRisk',
  'PrivacyFindingKind',
  'EmptyFormat',
  'AnonymizationStrategy',
  'PasteDataFormat',
  'WarningSeverity',
  'SmartReplacementRejectionReason',
  'PreflightMode',
  'ReleaseReadinessStatus',
  'ReleaseEvidenceStatus',
]

const structContracts = [
  'ColumnControl',
  'DetectionTrace',
  'DetectionTraceItem',
  'PrivacyFinding',
  'PrivacyEvidenceSummary',
  'ColumnMetadata',
  'HeadersData',
  'PasteAnalyzeData',
  'PasteTransformData',
  'QuickTransformData',
  'SampleTransform',
  'ColumnPreview',
  'PreviewWarning',
  'SmartReplacementEntry',
  'SmartReplacementRejectionCount',
  'PreviewData',
  'AnonymizeData',
  'PreflightParams',
  'PreflightData',
  'PrivacyReport',
  'ReleaseReadiness',
  'ReleaseEvidenceItem',
  'ColumnReleaseReport',
  'UtilityMetric',
]

const errors = []

for (const enumName of enumContracts) {
  const rustValues = rustEnumValues(enumName)
  const tsValues = tsUnionValues(enumName)
  compareSets(`${enumName} variants`, rustValues, tsValues)
}

for (const structName of structContracts) {
  const rustFields = rustStructFields(structName)
  const tsFields = tsInterfaceFields(structName)
  compareSets(`${structName} fields`, rustFields, tsFields)
}

if (errors.length > 0) {
  console.error('Contract check failed:')
  for (const error of errors) {
    console.error(`- ${error}`)
  }
  process.exit(1)
}

console.log(`Contract check passed for ${enumContracts.length} enums and ${structContracts.length} structs.`)

function rustEnumValues(name) {
  const body = matchBody(new RegExp(`pub enum ${name} \\{([\\s\\S]*?)\\n\\}`), rustTypes, `Rust enum ${name}`)
  return body
    .split('\n')
    .map((line) => line.replace(/\/\/.*$/, '').trim())
    .map((line) => line.match(/^([A-Z][A-Za-z0-9_]*)\b/)?.[1])
    .filter(Boolean)
    .map(camelCase)
}

function tsUnionValues(name) {
  const body = matchBody(
    new RegExp(`export type ${name} =([\\s\\S]*?)(?:\\nexport type |\\nexport interface |$)`),
    tsTypes,
    `TypeScript union ${name}`,
  )
  return [...body.matchAll(/'([^']+)'/g)].map((match) => match[1])
}

function rustStructFields(name) {
  const body = matchBody(new RegExp(`pub struct ${name} \\{([\\s\\S]*?)\\n\\}`), rustTypes, `Rust struct ${name}`)
  return body
    .split('\n')
    .map((line) => line.replace(/\/\/.*$/, '').trim())
    .map((line) => line.match(/^pub ([a-z][a-z0-9_]*)\s*:/)?.[1])
    .filter(Boolean)
    .map(camelCase)
}

function tsInterfaceFields(name) {
  const body = matchBody(
    new RegExp(`export interface ${name} \\{([\\s\\S]*?)\\n\\}`),
    tsTypes,
    `TypeScript interface ${name}`,
  )
  return body
    .split('\n')
    .map((line) => line.trim().match(/^([A-Za-z][A-Za-z0-9_]*)\??\s*:/)?.[1])
    .filter(Boolean)
}

function matchBody(regex, content, label) {
  const match = content.match(regex)
  if (!match) {
    errors.push(`Missing ${label}`)
    return ''
  }
  return match[1]
}

function compareSets(label, expectedValues, actualValues) {
  const expected = new Set(expectedValues)
  const actual = new Set(actualValues)
  const missing = [...expected].filter((value) => !actual.has(value))
  const extra = [...actual].filter((value) => !expected.has(value))

  if (missing.length > 0) {
    errors.push(`${label} missing in TypeScript: ${missing.join(', ')}`)
  }
  if (extra.length > 0) {
    errors.push(`${label} extra in TypeScript: ${extra.join(', ')}`)
  }
}

function camelCase(value) {
  if (value.includes('_')) {
    return value.replace(/_([a-z0-9])/g, (_, character) => character.toUpperCase())
  }
  return value.charAt(0).toLowerCase() + value.slice(1)
}
