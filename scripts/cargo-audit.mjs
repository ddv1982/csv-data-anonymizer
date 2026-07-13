#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import { resolveCargoSubcommand, resolveCommand } from './command-utils.mjs'

export const temporaryAuditExceptions = [
  {
    advisory: 'RUSTSEC-2026-0194',
    crate: 'quick-xml',
    version: '0.39.4',
    owner: '@ddv1982',
    rationale: 'Tauri plist is the sole remaining dependency path; application-owned XML uses quick-xml 0.41.0.',
    parentPrefix: 'plist v',
    expiresOn: '2026-10-01',
  },
  {
    advisory: 'RUSTSEC-2026-0195',
    crate: 'quick-xml',
    version: '0.39.4',
    owner: '@ddv1982',
    rationale: 'Tauri plist is the sole remaining dependency path; application-owned XML uses quick-xml 0.41.0.',
    parentPrefix: 'plist v',
    expiresOn: '2026-10-01',
  },
]

export function validateAuditExceptions(exceptions, now = new Date()) {
  const errors = []
  const advisories = new Set()
  const today = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate()))

  for (const [index, exception] of exceptions.entries()) {
    const label = exception?.advisory || `exception ${index + 1}`
    if (!/^RUSTSEC-\d{4}-\d{4}$/.test(exception?.advisory ?? '')) {
      errors.push(`${label}: advisory must match RUSTSEC-YYYY-NNNN`)
    } else if (advisories.has(exception.advisory)) {
      errors.push(`${label}: duplicate advisory exception`)
    } else {
      advisories.add(exception.advisory)
    }

    for (const field of ['crate', 'version', 'owner', 'rationale', 'parentPrefix']) {
      if (typeof exception?.[field] !== 'string' || exception[field].trim() === '') {
        errors.push(`${label}: ${field} must be a non-empty string`)
      }
    }

    if (!/^\d{4}-\d{2}-\d{2}$/.test(exception?.expiresOn ?? '')) {
      errors.push(`${label}: expiresOn must use YYYY-MM-DD`)
      continue
    }
    const expiry = new Date(`${exception.expiresOn}T00:00:00Z`)
    if (Number.isNaN(expiry.valueOf()) || expiry.toISOString().slice(0, 10) !== exception.expiresOn) {
      errors.push(`${label}: expiresOn is not a valid calendar date`)
    } else if (expiry <= today) {
      errors.push(`${label}: exception expired on ${exception.expiresOn}`)
    }
  }

  if (errors.length > 0) {
    throw new Error(`Invalid cargo-audit exceptions:\n- ${errors.join('\n- ')}`)
  }
}

export function validateIgnoredAuditFindings(exceptions, report) {
  const findings = report?.vulnerabilities?.list
  if (!Array.isArray(findings)) {
    throw new Error('Cannot validate cargo-audit exceptions: JSON report has no vulnerability list.')
  }

  const errors = []
  for (const exception of exceptions) {
    const matchingAdvisory = findings.filter(
      (finding) => finding?.advisory?.id === exception.advisory,
    )
    if (matchingAdvisory.length === 0) {
      errors.push(`${exception.advisory}: declared exception has no matching audit finding`)
      continue
    }

    for (const finding of matchingAdvisory) {
      const crate = finding?.package?.name
      const version = finding?.package?.version
      if (crate !== exception.crate || version !== exception.version) {
        errors.push(
          `${exception.advisory}: finding ${crate ?? '<unknown>'}@${version ?? '<unknown>'} does not match declared ${exception.crate}@${exception.version}`,
        )
      }
    }
  }

  if (errors.length > 0) {
    throw new Error(`Cargo-audit exceptions do not match current findings:\n- ${errors.join('\n- ')}`)
  }
}

function main() {
  try {
    validateAuditExceptions(temporaryAuditExceptions)
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    process.exit(1)
  }

  const auditRequired =
    process.argv.includes('--required') ||
    process.env.CI === 'true' ||
    process.env.CSV_ANONYMIZER_REQUIRE_CARGO_AUDIT === '1'
  const ignoreArgs = temporaryAuditExceptions.flatMap(({ advisory }) => ['--ignore', advisory])
  const audit = resolveCargoAuditCommand() || resolveCargoAuditSubcommand()

  if (!audit) {
    console.warn(`${auditRequired ? 'Cannot run' : 'Skipping'} cargo audit: cargo-audit is not installed.`)
    console.warn('Install with `cargo install cargo-audit` to enable Rust supply-chain auditing locally.')
    process.exit(auditRequired ? 1 : 0)
  }

  validateTemporaryAuditExceptionPaths(temporaryAuditExceptions)
  validateTemporaryAuditExceptionFindings(audit, temporaryAuditExceptions)

  const result = spawnSync(audit.command, [...audit.args, ...ignoreArgs], {
    cwd: process.cwd(),
    stdio: 'inherit',
    shell: false
  })

  process.exit(result.status ?? 1)
}

function resolveCargoAuditCommand() {
  const audit = resolveCommand('cargo-audit')
  if (!audit) return undefined

  const subcommandResult = spawnSync(audit.command, ['audit', '--version'], {
    cwd: process.cwd(),
    encoding: 'utf8',
    stdio: 'ignore',
    shell: false
  })

  if (subcommandResult.status !== 0) {
    console.error(`Found cargo-audit at ${audit.command}, but \`cargo-audit audit --version\` failed, so the binary appears broken.`)
    console.error('Reinstall it with `cargo install cargo-audit --force` or remove the broken binary from PATH.')
    process.exit(1)
  }

  return { command: audit.command, args: ['audit'] }
}

function resolveCargoAuditSubcommand() {
  const audit = resolveCargoSubcommand('audit')
  if (!audit) return undefined
  return { command: audit.command, args: audit.args }
}

function validateTemporaryAuditExceptionFindings(audit, exceptions) {
  const inventory = spawnSync(audit.command, [...audit.args, '--json'], {
    cwd: process.cwd(),
    encoding: 'utf8',
    maxBuffer: 10 * 1024 * 1024,
    shell: false,
  })

  let report
  try {
    report = JSON.parse(inventory.stdout)
  } catch {
    console.error('Cannot parse cargo-audit JSON while validating temporary exceptions.')
    if (inventory.stderr) console.error(inventory.stderr.trim())
    process.exit(1)
  }

  try {
    validateIgnoredAuditFindings(exceptions, report)
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    process.exit(1)
  }
}

function validateTemporaryAuditExceptionPaths(exceptions) {
  const paths = new Map()
  for (const exception of exceptions) {
    paths.set(`${exception.crate}@${exception.version}\0${exception.parentPrefix}`, exception)
  }

  for (const exception of paths.values()) {
    const dependency = `${exception.crate}@${exception.version}`
    const tree = spawnSync('cargo', ['tree', '-i', dependency, '--depth', '1', '--prefix', 'none', '--charset', 'ascii', '--color', 'never'], {
      cwd: process.cwd(),
      encoding: 'utf8',
      shell: false
    })

    if (tree.status !== 0) {
      console.error(`Cannot validate temporary ${dependency} audit exception path.`)
      process.exit(tree.status ?? 1)
    }

    const parents = tree.stdout
      .split(/\r?\n/)
      .map(line => line.trim())
      .filter(Boolean)
      .slice(1)

    if (parents.length !== 1 || !parents[0].startsWith(exception.parentPrefix)) {
      console.error(`Temporary ${dependency} audit exception must stay limited to parent ${exception.parentPrefix}.`)
      console.error(tree.stdout.trim())
      process.exit(1)
    }
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main()
}
