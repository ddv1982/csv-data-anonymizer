#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { resolveCargoSubcommand, resolveCommand } from './command-utils.mjs'

const required =
  process.argv.includes('--required') ||
  process.env.CI === 'true' ||
  process.env.CSV_ANONYMIZER_REQUIRE_CARGO_MACHETE === '1'
const extraArgs = process.argv.slice(2).filter((arg) => arg !== '--required')
const machete = resolveCargoMacheteCommand() || resolveCargoSubcommand('machete')

if (!machete) {
  console.warn(`${required ? 'Cannot run' : 'Skipping'} cargo machete: cargo-machete is not installed.`)
  console.warn('Install with `cargo install cargo-machete --locked --version 0.9.2` to scan unused Rust dependencies locally.')
  process.exit(required ? 1 : 0)
}

const result = spawnSync(machete.command, [...machete.args, ...extraArgs], {
  cwd: process.cwd(),
  stdio: 'inherit',
  shell: false,
})

process.exit(result.status ?? 1)

function resolveCargoMacheteCommand() {
  const machete = resolveCommand('cargo-machete')
  if (!machete) return undefined

  const result = spawnSync(machete.command, ['--version'], {
    cwd: process.cwd(),
    encoding: 'utf8',
    stdio: 'ignore',
    shell: false,
  })

  return result.status === 0 ? { command: machete.command, args: [] } : undefined
}
