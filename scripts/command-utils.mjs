import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { delimiter, join } from 'node:path'

export function resolveCargoSubcommand(name) {
  const cargo = resolveCommand('cargo')
  if (!cargo) return undefined

  const result = spawnSync(cargo.command, [name, '--version'], {
    cwd: process.cwd(),
    encoding: 'utf8',
    stdio: 'ignore',
    shell: false,
  })

  return result.status === 0 ? { command: cargo.command, args: [name] } : undefined
}

export function resolveCommand(command) {
  const pathEntries = (process.env.PATH ?? '').split(delimiter).filter(Boolean)
  const extensions =
    process.platform === 'win32'
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
