import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { checkContracts, enumContracts, limitContracts, structContracts } from '../check-contracts.mjs'

// The fixture is generated from the contract lists the gate itself exports, so it
// agrees with them by construction. A contract added to the gate is covered here
// without touching this file; a fixture hand-written today would instead rot into
// a stale subset and stop exercising the newest contracts precisely when they are
// the ones most likely to drift.
function generatedRustTypes() {
  const declarations = []

  for (const [rustName] of limitContracts) {
    declarations.push(`pub const ${rustName}: usize = 10;`)
  }

  for (const name of enumContracts) {
    declarations.push(
      '#[derive(Debug, Clone, Serialize, Deserialize)]',
      '#[serde(rename_all = "camelCase")]',
      `pub enum ${name} {`,
      '    SampleVariant,',
      '    OtherVariant,',
      '}',
    )
  }

  for (const name of structContracts) {
    declarations.push(
      '#[derive(Debug, Clone, Serialize, Deserialize)]',
      '#[serde(rename_all = "camelCase")]',
      `pub struct ${name} {`,
      '    pub sample_field: String,',
      '    pub other_field: u32,',
      '}',
    )
  }

  return `${declarations.join('\n')}\n`
}

function generatedTsTypes() {
  const unions = enumContracts.map((name) => `export type ${name} = 'sampleVariant' | 'otherVariant'`)
  const interfaces = structContracts.map(
    (name) => `export interface ${name} {\n  sampleField: string\n  otherField: number\n}`,
  )
  // Unions first, then interfaces: the gate terminates a union body at the next
  // `export type` or `export interface`, so the final union needs a successor.
  return `${[...unions, ...interfaces].join('\n\n')}\n`
}

function generatedTsDefaults() {
  return `${limitContracts.map(([, tsName]) => `export const ${tsName} = 10`).join('\n')}\n`
}

// `jobs` lands in src-tauri/src/jobs.rs, the third entry in the gate's rustSources,
// which makes it the natural place to plant a second declaration: it is read after
// the core types file, so the copy is the later one and a first-match lookup would
// never see it.
function fixture({
  coreTypes = generatedRustTypes(),
  jobs = '',
  types = generatedTsTypes(),
  defaults = generatedTsDefaults(),
} = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'csv-anonymizer-contracts-'))
  fs.mkdirSync(path.join(root, 'crates/csv-anonymizer-core/src'), { recursive: true })
  fs.mkdirSync(path.join(root, 'src-tauri/src/commands'), { recursive: true })
  fs.mkdirSync(path.join(root, 'src-tauri/src/local_ai'), { recursive: true })
  fs.mkdirSync(path.join(root, 'src-tauri/src/settings'), { recursive: true })
  fs.mkdirSync(path.join(root, 'frontend/src'), { recursive: true })

  fs.writeFileSync(path.join(root, 'crates/csv-anonymizer-core/src/types.rs'), coreTypes)
  fs.writeFileSync(path.join(root, 'src-tauri/src/jobs.rs'), jobs)
  fs.writeFileSync(path.join(root, 'src-tauri/src/commands/csv.rs'), '')
  fs.writeFileSync(path.join(root, 'src-tauri/src/local_ai/types.rs'), '')
  fs.writeFileSync(path.join(root, 'src-tauri/src/settings/model.rs'), '')
  fs.writeFileSync(path.join(root, 'frontend/src/types.ts'), types)
  fs.writeFileSync(path.join(root, 'frontend/src/defaults.ts'), defaults)
  return root
}

test('accepts Rust and TypeScript declarations that agree on every contract', (t) => {
  const root = fixture()
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const { errors, checked } = checkContracts(root)
  assert.deepEqual(errors, [])
  assert.deepEqual(checked, {
    enums: enumContracts.length,
    structs: structContracts.length,
    limits: limitContracts.length,
  })
})

test('rejects a contract struct declared in two sources, naming both sites', (t) => {
  const root = fixture({
    jobs: [
      '#[derive(Debug, Clone, Serialize, Deserialize)]',
      '#[serde(rename_all = "camelCase")]',
      'pub struct AppSettings {',
      '    pub sample_field: String,',
      '    pub other_field: u32,',
      '}',
      '',
    ].join('\n'),
  })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const { errors } = checkContracts(root)
  assert.equal(errors.length, 1)
  assert.match(errors[0], /^Rust struct AppSettings is declared 2 times across the checked sources /)
  assert.match(errors[0], /crates\/csv-anonymizer-core\/src\/types\.rs:\d+/)
  assert.match(errors[0], /src-tauri\/src\/jobs\.rs:3/)
})

test('rejects a contract enum declared in two sources', (t) => {
  const root = fixture({
    jobs: ['pub enum ThemeMode {', '    SampleVariant,', '}', ''].join('\n'),
  })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const { errors } = checkContracts(root)
  assert.equal(errors.length, 1)
  assert.match(errors[0], /^Rust enum ThemeMode is declared 2 times across the checked sources /)
})

test('rejects a contract limit const declared in two sources', (t) => {
  const root = fixture({ jobs: 'pub const MAX_SAMPLE_ROW_COUNT: usize = 42;\n' })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const { errors } = checkContracts(root)
  assert.equal(errors.length, 1)
  assert.match(errors[0], /^Rust const MAX_SAMPLE_ROW_COUNT is declared 2 times across the checked sources /)
})

// The scope decision: only names the gate actually looks up can be made ambiguous
// by a duplicate, because only those have a lookup to resolve wrongly. Two crates
// each owning an unrelated helper is normal Rust and must not fail the gate.
test('accepts a duplicated name that is not a contract', (t) => {
  const root = fixture({
    jobs: [
      'pub struct NotAContract {',
      '    pub sample_field: String,',
      '}',
      '',
      'pub enum AlsoNotAContract {',
      '    SampleVariant,',
      '}',
      '',
    ].join('\n'),
    coreTypes: `${generatedRustTypes()}\npub struct NotAContract {\n    pub other_field: u32,\n}\n\npub enum AlsoNotAContract {\n    OtherVariant,\n}\n`,
  })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  assert.deepEqual(checkContracts(root).errors, [])
})

// An indented copy is the case the two old resolvers disagreed about: the body
// lookup searched unanchored and found the indented copy, while the serde check
// searched anchored to a line start and found the real declaration. One resolver
// cannot disagree with itself, so this now reports plain ambiguity.
test('rejects a contract copy hidden inside an indented module', (t) => {
  const root = fixture({
    jobs: [
      '#[cfg(test)]',
      'mod tests {',
      '    pub struct AppSettings {',
      '        pub decoy: String,',
      '    }',
      '}',
      '',
    ].join('\n'),
  })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const { errors } = checkContracts(root)
  assert.equal(errors.length, 1)
  assert.match(errors[0], /^Rust struct AppSettings is declared 2 times across the checked sources /)
  assert.match(errors[0], /src-tauri\/src\/jobs\.rs:3/)
})

// A declaration the gate cannot resolve must be reported as absent, not as
// ambiguous, so the message points at the real problem.
test('reports a contract missing from Rust as missing rather than ambiguous', (t) => {
  const root = fixture({
    coreTypes: generatedRustTypes().replace('pub enum ThemeMode {', 'pub enum ThemeModeRenamed {'),
  })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const { errors } = checkContracts(root)
  assert.deepEqual(errors, ['Missing Rust enum ThemeMode'])
})

// The invariant that used to be only an assumption: the declaration index refuses
// `pub(crate)`, and because the index is the sole resolver, nothing downstream can
// resolve to a crate-private declaration either. If a future edit teaches any
// lookup to accept a visibility the index does not record, this stops being a
// clean "missing" and the test fails.
test('treats a pub(crate) contract declaration as missing, never as checked', (t) => {
  const root = fixture({
    coreTypes: generatedRustTypes().replace('pub struct AppSettings {', 'pub(crate) struct AppSettings {'),
  })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const { errors } = checkContracts(root)
  assert.deepEqual(errors, ['Missing Rust struct AppSettings'])
})

// Guards against the fix being weakened rather than merely present: the drift the
// gate existed to catch in the first place must still fail.
test('rejects an enum variant missing from the TypeScript union', (t) => {
  const root = fixture({
    types: generatedTsTypes().replace(
      "export type ThemeMode = 'sampleVariant' | 'otherVariant'",
      "export type ThemeMode = 'sampleVariant'",
    ),
  })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  assert.deepEqual(checkContracts(root).errors, ['ThemeMode variants missing in TypeScript: otherVariant'])
})

test('rejects a struct field missing from the TypeScript interface', (t) => {
  const root = fixture({
    types: generatedTsTypes().replace(
      'export interface AppSettings {\n  sampleField: string\n  otherField: number\n}',
      'export interface AppSettings {\n  sampleField: string\n}',
    ),
  })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  assert.deepEqual(checkContracts(root).errors, ['AppSettings fields missing in TypeScript: otherField'])
})

test('rejects a limit whose Rust and TypeScript values disagree', (t) => {
  const root = fixture({
    defaults: generatedTsDefaults().replace('maxSampleRowCount = 10', 'maxSampleRowCount = 999'),
  })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  assert.deepEqual(checkContracts(root).errors, [
    'MAX_SAMPLE_ROW_COUNT is 10 in Rust but maxSampleRowCount is 999 in TypeScript; ' +
      'the settings inputs would offer a range the engine does not accept',
  ])
})

// The serde check has to resolve the same declaration the field comparison used,
// which is the other half of the single-resolver property.
test('rejects a contract struct without the camelCase serde attribute', (t) => {
  const root = fixture({
    coreTypes: generatedRustTypes().replace(
      '#[serde(rename_all = "camelCase")]\npub struct AppSettings {',
      'pub struct AppSettings {',
    ),
  })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const { errors } = checkContracts(root)
  assert.equal(errors.length, 1)
  assert.match(errors[0], /^Rust struct AppSettings is missing #\[serde\(rename_all = "camelCase"\)\]/)
})

// Rewrites the first contract struct's body on both sides. The struct is taken from the
// gate's own list rather than named here, for the same reason the fixture is generated:
// a hand-picked name rots the day someone reorders the list.
function withFirstStructBody(rustBody, tsBody) {
  const name = structContracts[0]
  const rustSource = generatedRustTypes()
  const tsSource = generatedTsTypes()
  const rustTarget = `pub struct ${name} {\n    pub sample_field: String,\n    pub other_field: u32,\n}`
  const tsTarget = `export interface ${name} {\n  sampleField: string\n  otherField: number\n}`

  // That the substitution found its target, not that it changed the text — several tests
  // legitimately leave one side at the generated default. A helper that silently matched
  // nothing would leave every test below asserting against an untouched fixture, some of
  // them vacuously green; one clear failure here beats several confusing ones there.
  assert.ok(rustSource.includes(rustTarget), 'the generated Rust struct body changed shape')
  assert.ok(tsSource.includes(tsTarget), 'the generated TypeScript interface changed shape')

  return fixture({
    coreTypes: rustSource.replace(rustTarget, `pub struct ${name} {\n${rustBody}\n}`),
    types: tsSource.replace(tsTarget, `export interface ${name} {\n${tsBody}\n}`),
  })
}

test('rejects a skipped field TypeScript declares as always present', (t) => {
  // The regression this check exists for. `skip_serializing_if` omits the key, so the field
  // arrives as `undefined` on every payload with an empty list — and an earlier version of
  // this gate read only `Option<...>`, saw a plain `Vec`, and demanded exactly this unsound
  // declaration while rejecting the correct one.
  const root = withFirstStructBody(
    [
      '    #[serde(default, skip_serializing_if = "Vec::is_empty")]',
      '    pub sample_field: Vec<String>,',
      '    pub other_field: u32,',
    ].join('\n'),
    ['  sampleField: string[]', '  otherField: number'].join('\n'),
  )
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const { errors } = checkContracts(root)
  assert.equal(errors.length, 1)
  assert.match(errors[0], /sampleField is skipped on serialization when Vec::is_empty/)
})

test('accepts a skipped field TypeScript marks optional', (t) => {
  const root = withFirstStructBody(
    [
      '    #[serde(default, skip_serializing_if = "Vec::is_empty")]',
      '    pub sample_field: Vec<String>,',
      '    pub other_field: u32,',
    ].join('\n'),
    ['  sampleField?: string[]', '  otherField: number'].join('\n'),
  )
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  assert.deepEqual(checkContracts(root).errors, [])
})

test('rejects an Option field whose TypeScript type cannot hold null', (t) => {
  // Without `skip_serializing_if`, `None` is serialized as a present `null`, so the union
  // has to admit it. This is the opposite failure from the one above and needs the opposite
  // declaration, which is why the check reads the attributes and not just the type.
  const root = withFirstStructBody(
    ['    pub sample_field: Option<String>,', '    pub other_field: u32,'].join('\n'),
    ['  sampleField: string', '  otherField: number'].join('\n'),
  )
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const { errors } = checkContracts(root)
  assert.equal(errors.length, 1)
  assert.match(errors[0], /sampleField is Option<\.\.\.> in Rust .* does not admit null/)
})

test('rejects a guard against a value the wire cannot send', (t) => {
  const root = withFirstStructBody(
    ['    pub sample_field: String,', '    pub other_field: u32,'].join('\n'),
    ['  sampleField?: string | null', '  otherField: number'].join('\n'),
  )
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const { errors } = checkContracts(root)
  assert.equal(errors.length, 1)
  assert.match(errors[0], /sampleField is always present and never null on the wire/)
})

test('sees a field whose serde attribute rustfmt wrapped onto several lines', (t) => {
  // The round-three signature in a new form. Reading only the first line of the attribute
  // made `default,` look like a declaration: it flushed as a non-match, discarded the pending
  // attributes, and glued the rest onto the field line so the `pub …` regex never matched.
  // The field then reached NEITHER check — the gate failed on the correct declaration and
  // passed on one with the field deleted outright.
  const rustBody = [
    '    #[serde(',
    '        default,',
    '        skip_serializing_if = "Option::is_none"',
    '    )]',
    '    pub sample_field: Option<String>,',
    '    pub other_field: u32,',
  ].join('\n')

  const correct = withFirstStructBody(
    rustBody,
    ['  sampleField?: string | null', '  otherField: number'].join('\n'),
  )
  t.after(() => fs.rmSync(correct, { recursive: true, force: true }))
  assert.deepEqual(checkContracts(correct).errors, [])

  // And the unsound one still fails, rather than passing because the field went missing.
  const deleted = withFirstStructBody(rustBody, ['  otherField: number'].join('\n'))
  t.after(() => fs.rmSync(deleted, { recursive: true, force: true }))
  const { errors } = checkContracts(deleted)
  assert.equal(errors.length, 1)
  assert.match(errors[0], /fields missing in TypeScript: sampleField$/)
})

test('refuses the serde attributes it cannot reason about', (t) => {
  // Each of these changes or removes the wire key, which the nullability rule reads from the
  // field name and type alone. Guessing would make the gate demand a declaration that lies —
  // the exact failure it exists to prevent — so it stops instead.
  for (const [attribute, why] of [
    ['#[serde(skip)]', 'skip'],
    ['#[serde(default, skip_serializing)]', 'skip_serializing'],
    ['#[serde(rename = "wireName")]', 'rename'],
    ['#[serde(flatten)]', 'flatten'],
  ]) {
    const root = withFirstStructBody(
      [`    ${attribute}`, '    pub sample_field: String,', '    pub other_field: u32,'].join('\n'),
      ['  sampleField: string', '  otherField: number'].join('\n'),
    )
    t.after(() => fs.rmSync(root, { recursive: true, force: true }))

    const { errors } = checkContracts(root)
    assert.ok(
      errors.some((error) => error.includes(`#[serde(${why})]`)),
      `${attribute} must be refused, got: ${JSON.stringify(errors)}`,
    )
  }
})

test('sees a field whose type rustfmt wrapped onto the next line', (t) => {
  // A field the Rust parser misses is invisible to the *name* comparison as well as to the
  // nullability one, so it could drift out of TypeScript entirely without failing anything.
  // rustfmt produces this shape on its own once a declaration passes max_width.
  const rustBody = [
    '    pub sample_field:',
    '        std::collections::HashMap<String, Vec<usize>>,',
    '    pub other_field: u32,',
  ].join('\n')
  const matching = withFirstStructBody(
    rustBody,
    ['  sampleField: Record<string, number[]>', '  otherField: number'].join('\n'),
  )
  t.after(() => fs.rmSync(matching, { recursive: true, force: true }))
  assert.deepEqual(checkContracts(matching).errors, [])

  const missing = withFirstStructBody(rustBody, ['  otherField: number'].join('\n'))
  t.after(() => fs.rmSync(missing, { recursive: true, force: true }))
  const { errors } = checkContracts(missing)
  assert.equal(errors.length, 1)
  assert.match(errors[0], /fields missing in TypeScript: sampleField$/)
})
