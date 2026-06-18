#!/usr/bin/env node
import { appendFileSync, readFileSync } from 'node:fs'
import { basename } from 'node:path'

const reportPaths = process.argv.slice(2)
if (reportPaths.length === 0) {
  throw new Error('Usage: node scripts/write-linux-metadata-summary.mjs <report.json> [...]')
}

const lines = []
for (const reportPath of reportPaths) {
  const report = JSON.parse(readFileSync(reportPath, 'utf8'))
  lines.push(`## Linux Package Metadata: ${basename(reportPath)}`)
  lines.push('')
  lines.push(`Gate: ${report.ok ? 'PASS' : 'FAIL'}`)
  lines.push('')
  lines.push('| Package | Format | Component ID | License | Desktop ID | Binary | Release |')
  lines.push('| --- | --- | --- | --- | --- | --- | --- |')

  for (const entry of report.packages ?? []) {
    lines.push(
      [
        basename(entry.package ?? ''),
        entry.package_format ?? '',
        entry.component_id ?? '',
        entry.project_license ?? '',
        entry.launchable ?? '',
        entry.binary ?? '',
        entry.release_version ?? ''
      ]
        .map(markdownCell)
        .join(' | ')
        .replace(/^/, '| ')
        .replace(/$/, ' |')
    )
  }

  const errors = (report.packages ?? []).flatMap((entry) =>
    (entry.errors ?? []).map((error) => `${basename(entry.package ?? '')}: ${error}`)
  )
  if (errors.length > 0) {
    lines.push('')
    lines.push('Errors:')
    for (const error of errors) {
      lines.push(`- ${error}`)
    }
  }

  lines.push('')
}

const output = `${lines.join('\n')}\n`
if (process.env.GITHUB_STEP_SUMMARY) {
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, output)
} else {
  process.stdout.write(output)
}

function markdownCell(value) {
  return String(value).replace(/\|/g, '\\|').replace(/\n/g, ' ')
}
