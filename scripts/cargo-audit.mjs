#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { resolveCargoSubcommand, resolveCommand } from './command-utils.mjs'

const auditRequired =
  process.argv.includes('--required') ||
  process.env.CI === 'true' ||
  process.env.CSV_ANONYMIZER_REQUIRE_CARGO_AUDIT === '1'
const audit = resolveCargoAuditCommand() || resolveCargoSubcommand('audit')

if (!audit) {
  console.warn(`${auditRequired ? 'Cannot run' : 'Skipping'} cargo audit: cargo-audit is not installed.`)
  console.warn('Install with `cargo install cargo-audit` to enable Rust supply-chain auditing locally.')
  process.exit(auditRequired ? 1 : 0)
}

const result = spawnSync(audit.command, audit.args, {
  cwd: process.cwd(),
  stdio: 'inherit',
  shell: false
})

process.exit(result.status ?? 1)

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
