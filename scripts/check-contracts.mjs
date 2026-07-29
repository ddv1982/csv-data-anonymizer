#!/usr/bin/env node
import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'
import { pathToFileURL } from 'node:url'

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

// The sources are searched as one joined blob, so the separator has to be known
// in two places at once: the join itself, and the arithmetic that turns a blob
// offset back into a file and line for error messages.
const blobSeparator = '\n\n'

// The single definition of what counts as a Rust declaration this gate can read,
// used to build the declaration index that every lookup then resolves through.
// Nothing else in this file may go looking for a declaration in the blob; see
// resolveDeclaration for why that is a correctness property and not just tidiness.
//
// Leading indentation is allowed on purpose. A copy of a contract type sitting in
// a `mod tests` is still a copy, and one that appears earlier in the blob than the
// real declaration would otherwise be read as the real thing.
//
// `pub(crate)` is deliberately not matched. The frontend can only ever see types
// that are `pub`, so a crate-private twin is not a contract declaration at all;
// because the index is the only resolver, refusing to match it here means nothing
// downstream can resolve to it either, and a contract that is only ever declared
// `pub(crate)` is reported missing rather than silently checked.
const declarationPattern = /^([ \t]*)pub (enum|struct|const) ([A-Za-z_][A-Za-z0-9_]*)/

// Limits the frontend has to hold a copy of, because it enforces them in the
// settings inputs before any command is called. Unlike a field name, a wrong number
// here fails silently: the panel accepts a value the engine then rejects, or clamps
// one the engine would have honoured.
export const limitContracts = [
  ['MAX_SAMPLE_ROW_COUNT', 'maxSampleRowCount'],
  ['MAX_PREVIEW_SAMPLE_COUNT', 'maxPreviewSampleCount'],
]

// Exported so the tests can generate a fixture that covers every contract by
// construction: a contract added here is automatically covered there too, instead
// of the tests pinning a stale subset that quietly stops matching this list.
export const enumContracts = [
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

export const structContracts = [
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
  'ColumnValueDistribution',
  'UtilityMetric',
  'AppSettings',
  'AnalyzeResponse',
  'AnonymizeJobStatus',
  'LocalAiRequest',
  'LocalAiStatus',
  'LocalAiDownloadStatus',
]

export function checkContracts(root) {
  const errors = []

  // Read the sources individually and remember where each one lands in the blob,
  // so a declaration found in the blob can still be reported as file:line.
  const rustFiles = []
  let blobOffset = 0
  for (const source of rustSources) {
    const content = fs.readFileSync(path.join(root, source), 'utf8')
    rustFiles.push({ source, content, blobOffset })
    blobOffset += content.length + blobSeparator.length
  }
  const rustTypes = rustFiles.map((file) => file.content).join(blobSeparator)

  // Every place a Rust declaration lives, keyed by `${kind} ${name}`. Built once,
  // from the sources one at a time, because the joined blob on its own cannot say
  // which file a match came from or whether a second copy exists somewhere in it.
  const declarationSites = new Map()
  for (const { source, content, blobOffset: fileOffset } of rustFiles) {
    let lineOffset = 0
    for (const [index, line] of content.split('\n').entries()) {
      const declaration = line.match(declarationPattern)
      if (declaration) {
        const [, indent, kind, name] = declaration
        const key = `${kind} ${name}`
        const sites = declarationSites.get(key) ?? []
        sites.push({
          source,
          line: index + 1,
          // Offset of the `pub` token in the blob, and the declaration line from
          // `pub` to its end. Together these are everything a lookup needs, which
          // is what keeps the lookups from searching the blob for themselves.
          index: fileOffset + lineOffset + indent.length,
          header: line.slice(indent.length).trimEnd(),
        })
        declarationSites.set(key, sites)
      }
      lineOffset += line.length + 1
    }
  }

  const tsTypes = fs.readFileSync(path.join(root, 'frontend/src/types.ts'), 'utf8')
  const tsDefaults = fs.readFileSync(path.join(root, 'frontend/src/defaults.ts'), 'utf8')

  for (const enumName of enumContracts) {
    const site = resolveDeclaration('enum', enumName, `Rust enum ${enumName}`)
    assertSerdeCamelCase('enum', enumName, site)
    const rustValues = rustEnumValues(enumName, site)
    const tsValues = tsUnionValues(enumName)
    // The TypeScript lookup still runs when the Rust side did not resolve, so a
    // missing union is still reported, but the set diff is skipped: with nothing
    // credible to compare against it would restate the whole TypeScript side as
    // "extra" and bury the error that actually needs acting on.
    if (site) {
      compareSets(`${enumName} variants`, rustValues, tsValues)
    }
  }

  for (const structName of structContracts) {
    const site = resolveDeclaration('struct', structName, `Rust struct ${structName}`)
    assertSerdeCamelCase('struct', structName, site)
    const rustFields = rustStructFields(structName, site)
    const tsFields = tsInterfaceFields(structName)
    if (site) {
      compareSets(`${structName} fields`, rustFields, tsFields)
    }
  }

  for (const [rustName, tsName] of limitContracts) {
    const site = resolveDeclaration('const', rustName, `Rust const ${rustName}`)
    const rustValue = rustConstValue(rustName, site)
    const tsValue = tsConstValue(tsName)
    if (rustValue !== null && tsValue !== null && rustValue !== tsValue) {
      errors.push(
        `${rustName} is ${rustValue} in Rust but ${tsName} is ${tsValue} in TypeScript; ` +
          'the settings inputs would offer a range the engine does not accept',
      )
    }
  }

  return {
    errors,
    checked: {
      enums: enumContracts.length,
      structs: structContracts.length,
      limits: limitContracts.length,
    },
  }

  // The one and only way anything here locates a Rust declaration. Every lookup
  // goes through this, which is what makes two of them unable to disagree about
  // which declaration they are talking about.
  //
  // That used to be exactly the bug. The body lookups searched the blob
  // unanchored and took the first match, while the serde-attribute check searched
  // it anchored to the start of a line; given an indented copy of a contract type
  // earlier in the blob, the field comparison read the copy while the attribute
  // check read the real declaration and passed it. Two resolvers, two answers,
  // one green gate over a type nobody had actually checked.
  //
  // A name declared twice is refused rather than resolved. Guessing which copy is
  // the real one is what produced the silent drift, and comparing an arbitrary one
  // against TypeScript would only bury the ambiguity under a field diff nobody can
  // act on.
  function resolveDeclaration(kind, name, label) {
    const sites = declarationSites.get(`${kind} ${name}`) ?? []
    if (sites.length === 0) {
      errors.push(`Missing ${label}`)
      return null
    }
    if (sites.length > 1) {
      const where = sites.map((site) => `${site.source}:${site.line}`).join(', ')
      errors.push(
        `Rust ${kind} ${name} is declared ${sites.length} times across the checked sources (${where}); ` +
          'there is no way to tell which copy the frontend actually mirrors, so checking either one would ' +
          'leave the others free to change fields, drop variants or lose #[serde(rename_all = "camelCase")] ' +
          'while this gate still reports green',
      )
      return null
    }
    return sites[0]
  }

  // The body of a resolved declaration: opens at the `{` ending the declaration
  // line, closes at the first `}` in column 0. That is the shape every declaration
  // in rustSources actually has; anything else — a generic parameter, a tuple
  // struct — is reported missing, which is the loud direction to fail in.
  function rustDeclarationBody(kind, name, site, label) {
    if (!site) {
      return ''
    }
    if (site.header !== `pub ${kind} ${name} {`) {
      errors.push(`Missing ${label}`)
      return ''
    }
    const open = site.index + site.header.length - 1
    const close = rustTypes.indexOf('\n}', open)
    if (close === -1) {
      errors.push(`Missing ${label}`)
      return ''
    }
    return rustTypes.slice(open + 1, close)
  }

  function rustEnumValues(name, site) {
    return rustDeclarationBody('enum', name, site, `Rust enum ${name} body`)
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

  function rustStructFields(name, site) {
    return rustDeclarationBody('struct', name, site, `Rust struct ${name} body`)
      .split('\n')
      .map((line) => line.replace(/\/\/.*$/, '').trim())
      .map((line) => line.match(/^pub (?:r#)?([a-z][a-z0-9_]*)\s*:/)?.[1])
      .filter(Boolean)
      .map(camelCase)
  }

  function assertSerdeCamelCase(kind, name, site) {
    if (!site) {
      // A missing or ambiguous declaration is already reported by
      // resolveDeclaration with a clearer label.
      return
    }
    const precedingLines = rustTypes.slice(0, site.index).split('\n')
    precedingLines.pop() // drop the partial line the declaration starts on
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

  function rustConstValue(name, site) {
    if (!site) {
      return null
    }
    const prefix = `pub const ${name}: usize = `
    const literal = site.header.startsWith(prefix) && site.header.endsWith(';')
      ? site.header.slice(prefix.length, -1)
      : ''
    if (!/^[0-9_]+$/.test(literal)) {
      errors.push(`Missing Rust const ${name}`)
      return null
    }
    return Number(literal.replaceAll('_', ''))
  }

  function tsConstValue(name) {
    const match = tsDefaults.match(new RegExp(`export const ${name} = ([0-9_]+)`))
    if (!match) {
      errors.push(`Missing TypeScript const ${name} in frontend/src/defaults.ts`)
      return null
    }
    return Number(match[1].replaceAll('_', ''))
  }

  // Only the TypeScript side still matches a body by regex. It is a single file
  // rather than a joined blob, so there is no first-match-across-sources hazard
  // here, and duplicate `export interface` names would be a TypeScript error long
  // before this gate ran.
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
}

function camelCase(value) {
  if (value.includes('_')) {
    return value.replace(/_([a-z0-9])/g, (_, character) => character.toUpperCase())
  }
  return value.charAt(0).toLowerCase() + value.slice(1)
}

function main() {
  const { errors, checked } = checkContracts(process.cwd())
  if (errors.length > 0) {
    console.error('Contract check failed:')
    for (const error of errors) {
      console.error(`- ${error}`)
    }
    process.exit(1)
  }

  console.log(
    `Contract check passed for ${checked.enums} enums, ${checked.structs} structs ` +
      `and ${checked.limits} limits.`,
  )
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main()
}
