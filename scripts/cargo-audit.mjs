#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { resolveCargoSubcommand, resolveCommand } from './command-utils.mjs'

const auditRequired =
  process.argv.includes('--required') ||
  process.env.CI === 'true' ||
  process.env.CSV_ANONYMIZER_REQUIRE_CARGO_AUDIT === '1'
const temporaryAuditIgnores = ['RUSTSEC-2026-0194', 'RUSTSEC-2026-0195']
const temporaryAuditIgnoreArgs = temporaryAuditIgnores.flatMap(id => ['--ignore', id])
const audit = resolveCargoAuditCommand() || resolveCargoAuditSubcommand()

if (!audit) {
  console.warn(`${auditRequired ? 'Cannot run' : 'Skipping'} cargo audit: cargo-audit is not installed.`)
  console.warn('Install with `cargo install cargo-audit` to enable Rust supply-chain auditing locally.')
  process.exit(auditRequired ? 1 : 0)
}

validateTemporaryQuickXmlException()

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

  return { command: audit.command, args: ['audit', ...temporaryAuditIgnoreArgs] }
}

function resolveCargoAuditSubcommand() {
  const audit = resolveCargoSubcommand('audit')
  if (!audit) return undefined
  return { command: audit.command, args: [...audit.args, ...temporaryAuditIgnoreArgs] }
}

function validateTemporaryQuickXmlException() {
  const tree = spawnSync('cargo', ['tree', '-i', 'quick-xml@0.39.4', '--depth', '1', '--prefix', 'none', '--charset', 'ascii', '--color', 'never'], {
    cwd: process.cwd(),
    encoding: 'utf8',
    shell: false
  })

  if (tree.status !== 0) {
    console.error('Cannot validate temporary quick-xml audit exception path.')
    process.exit(tree.status ?? 1)
  }

  const parents = tree.stdout
    .split(/\r?\n/)
    .map(line => line.trim())
    .filter(Boolean)
    .slice(1)

  if (parents.length !== 1 || !parents[0].startsWith('plist v')) {
    console.error('Temporary quick-xml audit exception must stay limited to the Tauri plist dependency path.')
    console.error(tree.stdout.trim())
    process.exit(1)
  }
}
