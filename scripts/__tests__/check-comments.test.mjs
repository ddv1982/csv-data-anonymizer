import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { checkComments, commentBlocks } from '../check-comments.mjs'

function fixture(source, { name = 'lib.rs' } = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'csv-anonymizer-comments-'))
  const dir = path.join(root, 'crates', 'core', 'src')
  fs.mkdirSync(dir, { recursive: true })
  fs.writeFileSync(path.join(dir, name), source)
  return root
}

test('rejects a comment that describes a previous version of the code', (t) => {
  const root = fixture('/// The gate used to accept an empty domain.\nfn parse() {}\n')
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const { errors } = checkComments(root)
  assert.equal(errors.length, 1)
  assert.match(errors[0], /previous version of the code/)
})

test('accepts the same fact stated in the present tense', (t) => {
  const root = fixture('/// An empty domain must be rejected: it merges two classes.\nfn parse() {}\n')
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  assert.deepEqual(checkComments(root).errors, [])
})

/**
 * "no longer" describes current behaviour everywhere in this repository, so matching it
 * would flag load-bearing text. This test is what stops someone "improving" the pattern
 * list by adding it.
 */
test('does not treat "no longer" as narration', (t) => {
  const root = fixture('/// `1800FLOWERS` is no longer recognized, while `+1800FLOWERS` is.\nfn parse() {}\n')
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  assert.deepEqual(checkComments(root).errors, [])
})

test('ignores narration inside a test module, where it documents the pinned bug', (t) => {
  const root = fixture(
    ['fn parse() {}', '', '#[cfg(test)]', 'mod tests {', '    /// They used to disagree.', '    fn case() {}', '}', ''].join('\n'),
  )
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  assert.deepEqual(checkComments(root).errors, [])
})

test('warns about a long block without failing the run', (t) => {
  const long = Array.from({ length: 21 }, (_, index) => `/// line ${index}`).join('\n')
  const root = fixture(`${long}\nfn parse() {}\n`)
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  const { errors, warnings } = checkComments(root)
  assert.deepEqual(errors, [])
  assert.equal(warnings.length, 1)
  assert.match(warnings[0], /21 lines/)
})

test('a block exactly at the threshold is not warned about', (t) => {
  const exact = Array.from({ length: 20 }, (_, index) => `/// line ${index}`).join('\n')
  const root = fixture(`${exact}\nfn parse() {}\n`)
  t.after(() => fs.rmSync(root, { recursive: true, force: true }))

  assert.deepEqual(checkComments(root).warnings, [])
})

test('splits consecutive comment runs into separate blocks', () => {
  const blocks = commentBlocks(['// one', '// two', 'fn a() {}', '// three', 'fn b() {}', ''].join('\n'))
  assert.deepEqual(
    blocks.map((block) => ({ start: block.start, count: block.lines.length })),
    [
      { start: 1, count: 2 },
      { start: 4, count: 1 },
    ],
  )
})
