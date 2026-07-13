import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { checkDocs } from '../check-docs.mjs'

function fixture({ readme = '', guide = '', scripts = { 'known:check': 'node scripts/known.mjs' } } = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'csv-anonymizer-docs-'))
  fs.mkdirSync(path.join(root, 'docs'))
  fs.mkdirSync(path.join(root, 'scripts'))
  fs.writeFileSync(path.join(root, 'package.json'), JSON.stringify({ scripts }))
  fs.writeFileSync(path.join(root, 'README.md'), readme)
  fs.writeFileSync(path.join(root, 'docs', 'guide.md'), guide)
  fs.writeFileSync(path.join(root, 'scripts', 'known.mjs'), '')
  return root
}

test('accepts defined npm scripts and existing direct repository commands', (t) => {
  const root = fixture({ guide: '```bash\nnpm run known:check\nnode ./scripts/known.mjs\n```\n' })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  assert.deepEqual(checkDocs(root).errors, [])
})

test('rejects undefined npm scripts', (t) => {
  const root = fixture({ readme: 'Run `npm run missing:check` before release.\n' })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  assert.deepEqual(checkDocs(root).errors, [
    'README.md: documented npm script "missing:check" is not defined in package.json',
  ])
})

test('rejects missing script files in documented shell commands', (t) => {
  const root = fixture({ guide: '```shell\npython3 scripts/missing.py --strict\n```\n' })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  assert.deepEqual(checkDocs(root).errors, [
    'docs/guide.md: documented repository command references missing file "scripts/missing.py"',
  ])
})

test('rejects direct commands that escape the scripts directory', (t) => {
  const root = fixture({ guide: '```bash\nnode scripts/../package.json\n```\n' })
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  assert.deepEqual(checkDocs(root).errors, [
    'docs/guide.md: documented repository command references missing file "scripts/../package.json"',
  ])
})
