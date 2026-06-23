#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, relative } from 'node:path'

const projectRoot = process.cwd()
const binary = findOrBuildBinary()
const tempDir = mkdtempSync(join(tmpdir(), 'csv-anonymizer-smoke-'))
const input = join(projectRoot, 'tests', 'fixtures', 'sample.csv')
const output = join(tempDir, 'sample-smoke-output.csv')
const sourceEmails = extractEmails(readFileSync(input, 'utf8'))

if (sourceEmails.length === 0) {
  throw new Error(`smoke fixture contains no source email values: ${relative(projectRoot, input)}`)
}

try {
  run(binary, ['--smoke-anonymize', input, output])
  if (!existsSync(output)) {
    throw new Error('smoke command did not create output CSV')
  }
  const outputCsv = readFileSync(output, 'utf8')
  const retainedEmails = sourceEmails.filter((email) => outputCsv.includes(email))
  if (retainedEmails.length > 0) {
    throw new Error(`smoke output still contains source email value(s): ${retainedEmails.join(', ')}`)
  }
  console.log(`Rust smoke OK: removed ${sourceEmails.length} fixture email value(s) from ${relative(projectRoot, input)} -> ${output}`)
} finally {
  rmSync(tempDir, { recursive: true, force: true })
}

function extractEmails(csv) {
  return [...new Set(csv.match(/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/gi) ?? [])].sort()
}

function findOrBuildBinary() {
  if (process.env.CSV_ANONYMIZER_BINARY) {
    if (!existsSync(process.env.CSV_ANONYMIZER_BINARY)) {
      throw new Error(`CSV_ANONYMIZER_BINARY does not exist: ${process.env.CSV_ANONYMIZER_BINARY}`)
    }
    return process.env.CSV_ANONYMIZER_BINARY
  }

  run('cargo', ['build', '-p', 'csv-anonymizer-app'])
  const built = join(projectRoot, 'target', 'debug', 'csv-anonymizer')
  if (!existsSync(built)) {
    throw new Error(`Rust smoke binary was not built at ${relative(projectRoot, built)}`)
  }
  return built
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: projectRoot,
    stdio: 'inherit',
    encoding: 'utf8'
  })
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} failed with exit code ${result.status ?? 'unknown'}`)
  }
}
