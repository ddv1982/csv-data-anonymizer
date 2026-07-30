#!/usr/bin/env node
import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'
import { pathToFileURL } from 'node:url'

/**
 * Comments here explain WHY and name the failure prevented — in the present tense.
 *
 * Two rules, and they exist for different reasons.
 *
 * The error rule: a comment that describes a *previous version of the code* is a changelog
 * entry in the wrong place. Git already holds that, with the diff attached. Left unchecked
 * it accumulates — an audit of this crate found 19 such sites anchoring roughly 500 lines,
 * one of which corrected an earlier version of its own comment.
 *
 * The warning rule: comment mass here is a tail phenomenon. The median block is 4 lines,
 * but blocks of 20+ lines hold a quarter of every comment line in the crate. Length is not
 * itself a defect — a calibration table or a security constraint can earn it — so this
 * warns rather than fails, and the threshold is set where the tail actually starts.
 */

/**
 * Phrases that describe what the code *used to* do.
 *
 * Deliberately does NOT include "no longer". Every occurrence of it in this repository
 * describes current behaviour ("`1800FLOWERS` is no longer recognized", "cancelling needs a
 * job id the client no longer has"), so matching it would flag load-bearing text and train
 * people to ignore this check.
 */
const NARRATION_PATTERNS = [
  /\bused to\b/i,
  /\bpreviously (?:did|was|were|fell|said|had)\b/i,
  /\ban earlier version\b/i,
  /\bthat reasoning was wrong\b/i,
  /\bit did not always\b/i,
]

/** Where the long tail of comment blocks begins. Above this a block must justify itself. */
const LONG_BLOCK_LINES = 20

const COMMENT_LINE = /^\s*(?:\/\/\/|\/\/!|\/\/)/

function rustSources(root) {
  const roots = [path.join(root, 'crates'), path.join(root, 'src-tauri', 'src')]
  const found = []
  for (const start of roots) {
    if (!fs.existsSync(start)) continue
    const stack = [start]
    while (stack.length > 0) {
      const current = stack.pop()
      for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
        const full = path.join(current, entry.name)
        if (entry.isDirectory()) {
          // `target` is build output; `tests` directories are the suite's own prose, which
          // is allowed to narrate what a regression looked like.
          if (entry.name !== 'target' && entry.name !== 'tests') stack.push(full)
        } else if (entry.name.endsWith('.rs') && entry.name !== 'tests.rs') {
          found.push(full)
        }
      }
    }
  }
  return found.sort()
}

/**
 * Comment blocks in one file, as `{ start, lines }`.
 *
 * A block is consecutive comment lines. Test modules are skipped from `#[cfg(test)]`
 * onward: a test doc that says "they used to disagree in both directions" is describing
 * the bug the test pins, which is exactly where that sentence belongs.
 */
export function commentBlocks(source) {
  const lines = source.split('\n')
  const blocks = []
  let current = null
  for (const [index, line] of lines.entries()) {
    if (/^\s*#\[cfg\(test\)\]/.test(line)) break
    if (COMMENT_LINE.test(line)) {
      if (current === null) current = { start: index + 1, lines: [] }
      current.lines.push(line)
    } else if (current !== null) {
      blocks.push(current)
      current = null
    }
  }
  if (current !== null) blocks.push(current)
  return blocks
}

export function checkComments(root) {
  const errors = []
  const warnings = []

  for (const file of rustSources(root)) {
    const relative = path.relative(root, file)
    for (const block of commentBlocks(fs.readFileSync(file, 'utf8'))) {
      for (const [offset, line] of block.lines.entries()) {
        const pattern = NARRATION_PATTERNS.find((candidate) => candidate.test(line))
        if (pattern !== undefined) {
          errors.push(
            `${relative}:${block.start + offset}: comment describes a previous version of the code. ` +
              `State the rule that binds now, in the present tense, and let git hold the history.`,
          )
        }
      }
      if (block.lines.length > LONG_BLOCK_LINES) {
        warnings.push(
          `${relative}:${block.start}: comment block is ${block.lines.length} lines (over ${LONG_BLOCK_LINES}). ` +
            `Keep it if it names a failure or carries evidence; move measurements to docs/calibration.md.`,
        )
      }
    }
  }

  return { errors, warnings }
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { errors, warnings } = checkComments(process.cwd())
  for (const warning of warnings) console.warn(`warning: ${warning}`)
  for (const error of errors) console.error(`error: ${error}`)
  if (errors.length > 0) {
    console.error(`\n${errors.length} comment(s) describe a previous version of the code.`)
    process.exit(1)
  }
  console.log(`comments: no past-tense narration found (${warnings.length} long block(s) noted)`)
}
