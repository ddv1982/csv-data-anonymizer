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
  'FormatEvidenceBasis',
  'SemanticSpecificity',
  'SemanticStatus',
  'RedactionPlaceholderSource',
  'PasteDataFormat',
  'DetectionCoverageUnit',
  'WarningSeverity',
  'SmartReplacementRejectionReason',
  'PreflightMode',
  'ReleaseReadinessStatus',
  'ReleaseEvidenceStatus',
  'MatchedPart',
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
  'FormatEvidence',
  'SemanticDecision',
  'PrivacyDecision',
  'RedactionDecision',
  'ColumnEvidenceProfile',
  'ColumnMetadata',
  'HeadersData',
  'DetectionCoverageSummary',
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
  'RowUniquenessSummary',
  'MatchedColumn',
  'DropColumnEffect',
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
  // Memoized because three callers want the same interface body and `matchBody` reports a
  // missing one. Without the cache, renaming a single TypeScript interface printed the same
  // "Missing TypeScript interface" line three times over the one that needed acting on.
  const tsInterfaceCache = new Map()

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
      compareNullability(structName, site)
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
    return [...rustStructFieldTypes(name, site).keys()]
  }

  /// Every field of a Rust struct, with its type text and the attributes above it.
  ///
  /// Declarations are accumulated across lines rather than matched one line at a time,
  /// because rustfmt wraps `pub field:` and its type onto separate lines once the
  /// declaration passes `max_width`. A single-line pattern skips such a field silently, and
  /// a field this function does not return is invisible to the *name* comparison too — so
  /// the wrapped field would reach neither check and could drift out of TypeScript
  /// unnoticed, which is the one thing this gate exists to prevent.
  function rustStructFieldTypes(name, site) {
    const fields = new Map()
    let attributes = []
    let attribute = ''
    let declaration = ''

    const flush = () => {
      const match = declaration.match(/^pub (?:r#)?([a-z][a-z0-9_]*)\s*:\s*(.+?),?$/)
      if (match) {
        const field = camelCase(match[1])
        assertSupportedSerdeAttributes(name, field, attributes)
        fields.set(field, { type: match[2].trim(), attributes })
      }
      attributes = []
      declaration = ''
    }

    for (const line of rustDeclarationBody('struct', name, site, `Rust struct ${name} body`)
      .split('\n')
      .map((line) => line.replace(/\/\/.*$/, '').trim())) {
      if (line === '') {
        continue
      }
      // Attributes only ever precede a declaration, so they accumulate until one completes.
      // Accumulated across lines for the same reason declarations are: rustfmt wraps a long
      // `#[serde(...)]` onto several lines, and reading only the first swallowed the rest as
      // a declaration, discarded the pending attributes, and lost the field from BOTH checks.
      if (declaration === '' && (attribute !== '' || line.startsWith('#['))) {
        attribute = attribute === '' ? line : `${attribute} ${line}`
        if (bracketDepth(attribute) === 0) {
          attributes.push(attribute)
          attribute = ''
        }
        continue
      }
      declaration = declaration === '' ? line : `${declaration} ${line}`
      // Complete once the generic brackets balance and the comma arrives. Field types hold
      // no `<` or `>` of their own beyond generics, so counting is enough here and a real
      // parser would be more machinery than the shapes in these files justify.
      if (angleDepth(declaration) === 0 && declaration.endsWith(',')) {
        flush()
      }
    }
    // A final field without its trailing comma is not something rustfmt emits, but reading
    // it costs one line and losing it silently would cost a contract.
    flush()

    return fields
  }

  // Bracket nesting of an accumulating `#[...]`, so a wrapped attribute is one unit.
  function bracketDepth(text) {
    return [...text].reduce(
      (open, character) =>
        character === '[' || character === '('
          ? open + 1
          : character === ']' || character === ')'
            ? open - 1
            : open,
      0,
    )
  }

  // Generic nesting of an accumulating field declaration. Field types in these files hold no
  // `<` or `>` of their own beyond generics; a real parser would be more machinery than the
  // shapes here justify, and `assertSupportedSerdeAttributes` refuses the cases where a
  // guess would be silently wrong rather than loudly missing.
  function angleDepth(text) {
    return [...text].reduce(
      (open, character) =>
        character === '<' ? open + 1 : character === '>' ? open - 1 : open,
      0,
    )
  }

  /// Refuses the serde attributes this gate cannot reason about.
  ///
  /// `compareNullability` answers one question — can TypeScript hold everything the wire can
  /// send? — from `skip_serializing_if` and `Option`. Four attributes change the answer and
  /// are invisible to it: `skip` and `skip_serializing` drop the key unconditionally,
  /// field-level `rename` changes the key's name, and `flatten` hoists a whole struct's keys
  /// in place of its own. Each would make the gate demand a declaration that lies about the
  /// wire, which is the exact failure this check exists to prevent — so an unrecognised one
  /// is a hard error rather than a wrong verdict. None is present in a registered struct
  /// today; this is what keeps that true.
  function assertSupportedSerdeAttributes(name, field, attributes) {
    const serde = attributes.filter((line) => line.startsWith('#[serde'))
    for (const [pattern, why] of [
      [/\bskip\s*[,)\]]/, 'skip'],
      [/\bskip_serializing\s*[,)\]]/, 'skip_serializing'],
      [/\brename\s*=/, 'rename'],
      [/\bflatten\b/, 'flatten'],
    ]) {
      if (serde.some((line) => pattern.test(line))) {
        errors.push(
          `${name}.${field} carries #[serde(${why})], which this gate cannot reason about; ` +
            'the wire key it produces is not derivable from the field name and type, so the ' +
            'nullability rule would be applied to the wrong key',
        )
      }
    }
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

  /// Fields TypeScript declares narrower than the wire can actually deliver.
  ///
  /// The set comparison above checks names and nothing else, which is enough to catch a
  /// field added on one side only and blind to the change that breaks a running app: a
  /// field the engine may omit, declared in TypeScript as always present. Nothing fails
  /// until a report arrives without it and the view reads a property off `undefined`.
  ///
  /// The question is deliberately one-directional — **can TypeScript hold everything the
  /// wire can send?** — and it is answered from the serde attributes rather than from the
  /// Rust type alone, because the type alone gets it wrong in both directions:
  ///
  /// - `#[serde(skip_serializing_if = ...)]` omits the key entirely, so the field arrives
  ///   as `undefined` and TypeScript must mark it `?`. `Vec` fields do this too, which is
  ///   how an earlier version of this check came to *demand* the unsound declaration: it
  ///   read only `Option<...>`, saw a plain `Vec`, and reported three correctly-optional
  ///   TypeScript fields as guarding against "a value that can never arrive" — a value
  ///   that arrives absent on every report with an empty list.
  /// - `Option<T>` without that attribute sends the key as `null`, which is present. Those
  ///   need a `| null` in the union and do not need the `?`.
  ///
  /// Optionality only. Comparing `usize` against `number` properly would mean parsing two
  /// type languages, and a half-parser that silently passes what it cannot read is worse
  /// than the honest gap it replaces.
  function compareNullability(name, site) {
    const rustFields = rustStructFieldTypes(name, site)
    const tsFields = tsInterfaceFieldTypes(name)

    for (const [field, rust] of rustFields) {
      const declared = tsFields.get(field)
      if (declared === undefined) {
        // Already reported by the set comparison, with a better label.
        continue
      }
      const skipCondition = rust.attributes
        .join(' ')
        .match(/skip_serializing_if\s*=\s*"([^"]+)"/)?.[1]
      const tsAdmitsNull = /(^|\|)\s*(null|undefined)\s*(\||$)/.test(declared.type)

      if (skipCondition) {
        if (!declared.optional) {
          errors.push(
            `${name}.${field} is skipped on serialization when ${skipCondition}, so the key is ` +
              'absent from the JSON, but TypeScript declares it as always present; the view would ' +
              'read a property off undefined',
          )
        }
        continue
      }
      if (/^Option\s*</.test(rust.type)) {
        if (!tsAdmitsNull) {
          errors.push(
            `${name}.${field} is Option<...> in Rust and is serialized as null when None, but the ` +
              'TypeScript type does not admit null',
          )
        }
        continue
      }
      if (declared.optional || tsAdmitsNull) {
        errors.push(
          `${name}.${field} is always present and never null on the wire, but TypeScript declares ` +
            'it optional or nullable; the view guards against a value that cannot arrive, so the ' +
            'guard can never be exercised',
        )
      }
    }
  }

  function tsInterfaceFields(name) {
    return [...tsInterfaceFieldTypes(name).keys()]
  }

  function tsInterfaceFieldTypes(name) {
    const cached = tsInterfaceCache.get(name)
    if (cached) {
      return cached
    }
    const body = matchBody(
      new RegExp(`export interface ${name} \\{([\\s\\S]*?)\\n\\}`),
      tsTypes,
      `TypeScript interface ${name}`,
    )
    const fields = new Map()
    for (const line of body.split('\n').map((line) => line.trim())) {
      const match = line.match(/^([A-Za-z][A-Za-z0-9_]*)(\?)?\s*:\s*(.+?)$/)
      if (match) {
        fields.set(match[1], { optional: match[2] === '?', type: match[3].trim() })
      }
    }
    tsInterfaceCache.set(name, fields)
    return fields
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
