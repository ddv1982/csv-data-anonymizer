#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { delimiter, join } from 'node:path'

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

function resolveCargoSubcommand(name) {
  const cargo = resolveCommand('cargo')
  if (!cargo) return undefined

  const result = spawnSync(cargo.command, ['audit', '--version'], {
    cwd: process.cwd(),
    encoding: 'utf8',
    stdio: 'ignore',
    shell: false
  })

  return result.status === 0 ? { command: cargo.command, args: [name] } : undefined
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

  return {
    command: audit.command,
    args: subcommandResult.status === 0 ? ['audit'] : []
  }
}

function resolveCommand(command) {
  const pathEntries = (process.env.PATH ?? '').split(delimiter).filter(Boolean)
  const extensions = process.platform === 'win32'
    ? (process.env.PATHEXT ?? '.EXE;.CMD;.BAT;.COM').split(';')
    : ['']

  for (const directory of pathEntries) {
    for (const extension of extensions) {
      const candidate = join(directory, `${command}${extension}`)
      if (existsSync(candidate)) {
        return { command: candidate, args: [] }
      }
    }
  }

  return undefined
}
