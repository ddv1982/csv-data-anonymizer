#!/usr/bin/env node
import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'

const root = process.cwd()

// Every Rust source that declares a type the frontend mirrors. The core crate
// owns the analysis/transform vocabulary; the Tauri crate owns the settings,
// job and Local AI surfaces, which reach the frontend over IPC just the same
// and drift just as silently when they are left unchecked.
const rustSources = [
  'crates/csv-anonymizer-core/src/types.rs',
  'src-tauri/src/commands/csv.rs',
  'src-tauri/src/jobs.rs',
  'src-tauri/src/local_ai/types.rs',
  'src-tauri/src/settings/model.rs',
]

const rustTypes = rustSources
  .map((source) => fs.readFileSync(path.join(root, source), 'utf8'))
  .join('\n\n')
const tsTypes = fs.readFileSync(path.join(root, 'frontend/src/types.ts'), 'utf8')
const tsDefaults = fs.readFileSync(path.join(root, 'frontend/src/defaults.ts'), 'utf8')

// Limits the frontend has to hold a copy of, because it enforces them in the
// settings inputs before any command is called. Unlike a field name, a wrong number
// here fails silently: the panel accepts a value the engine then rejects, or clamps
// one the engine would have honoured.
const limitContracts = [
  ['MAX_SAMPLE_ROW_COUNT', 'maxSampleRowCount'],
  ['MAX_PREVIEW_SAMPLE_COUNT', 'maxPreviewSampleCount'],
]

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
  'ThemeMode',
  'AnonymizeJobState',
  'LocalAiDownloadState',
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
  'PreviewParams',
  'PastePreviewParams',
  'PasteTransformParams',
  'PreflightData',
  'PrivacyReport',
  'ReleaseReadiness',
  'ReleaseEvidenceItem',
  'ColumnReleaseReport',
  'UtilityMetric',
  'AppSettings',
  'AnalyzeResponse',
  'AnonymizeJobStatus',
  'LocalAiRequest',
  'LocalAiStatus',
  'LocalAiDownloadStatus',
]

const errors = []

for (const enumName of enumContracts) {
  assertSerdeCamelCase('enum', enumName)
  const rustValues = rustEnumValues(enumName)
  const tsValues = tsUnionValues(enumName)
  compareSets(`${enumName} variants`, rustValues, tsValues)
}

for (const structName of structContracts) {
  assertSerdeCamelCase('struct', structName)
  const rustFields = rustStructFields(structName)
  const tsFields = tsInterfaceFields(structName)
  compareSets(`${structName} fields`, rustFields, tsFields)
}

for (const [rustName, tsName] of limitContracts) {
  const rustValue = rustConstValue(rustName)
  const tsValue = tsConstValue(tsName)
  if (rustValue !== null && tsValue !== null && rustValue !== tsValue) {
    errors.push(
      `${rustName} is ${rustValue} in Rust but ${tsName} is ${tsValue} in TypeScript; ` +
        'the settings inputs would offer a range the engine does not accept',
    )
  }
}

if (errors.length > 0) {
  console.error('Contract check failed:')
  for (const error of errors) {
    console.error(`- ${error}`)
  }
  process.exit(1)
}

console.log(
  `Contract check passed for ${enumContracts.length} enums, ${structContracts.length} structs ` +
    `and ${limitContracts.length} limits.`,
)

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
    .map((line) => line.match(/^pub (?:r#)?([a-z][a-z0-9_]*)\s*:/)?.[1])
    .filter(Boolean)
    .map(camelCase)
}

function assertSerdeCamelCase(kind, name) {
  const declaration = rustTypes.match(new RegExp(`^pub ${kind} ${name} [\\{(]`, 'm'))
  if (!declaration) {
    // The missing declaration itself is reported by matchBody with a clearer label.
    return
  }
  const precedingLines = rustTypes.slice(0, declaration.index).split('\n')
  precedingLines.pop() // drop the empty remainder after the final newline
  const attributeLines = []
  for (let index = precedingLines.length - 1; index >= 0; index -= 1) {
    const line = precedingLines[index].trim()
    if (line.startsWith('#[') || line.startsWith('//')) {
      attributeLines.push(line)
      continue
    }
    break
  }
  const hasCamelCaseRename = attributeLines.some((line) =>
    /#\[serde\(.*rename_all\s*=\s*"camelCase"/.test(line),
  )
  if (!hasCamelCaseRename) {
    errors.push(
      `Rust ${kind} ${name} is missing #[serde(rename_all = "camelCase")]; ` +
        'the contract comparison assumes camelCase serialization, so without the attribute it would silently pass while the wire format stays snake_case',
    )
  }
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

function rustConstValue(name) {
  const match = rustTypes.match(new RegExp(`pub const ${name}: usize = ([0-9_]+);`))
  if (!match) {
    errors.push(`Missing Rust const ${name}`)
    return null
  }
  return Number(match[1].replaceAll('_', ''))
}

function tsConstValue(name) {
  const match = tsDefaults.match(new RegExp(`export const ${name} = ([0-9_]+)`))
  if (!match) {
    errors.push(`Missing TypeScript const ${name} in frontend/src/defaults.ts`)
    return null
  }
  return Number(match[1].replaceAll('_', ''))
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
