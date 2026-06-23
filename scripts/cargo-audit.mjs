#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { delimiter, join } from 'node:path'

const audit = resolveCommand('cargo-audit') || resolveCargoSubcommand('audit')

if (!audit) {
  console.warn('Skipping cargo audit: cargo-audit is not installed.')
  console.warn('Install with `cargo install cargo-audit` to enable Rust supply-chain auditing locally.')
  process.exit(0)
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
