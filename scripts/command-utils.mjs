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

export function readOption(args, name) {
  const index = args.indexOf(name)
  if (index === -1) return undefined

  const value = args[index + 1]
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`)
  }
  return value
}

export function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? process.cwd(),
    env: { ...process.env, ...(options.env ?? {}) },
    stdio: options.stdio ?? 'inherit',
    encoding: options.encoding,
    shell: false,
  })

  if (result.status === 0 || options.allowFailure) return result
  if (result.stdout) process.stdout.write(result.stdout)
  if (result.stderr) process.stderr.write(result.stderr)
  const error = new Error(`${command} ${args.join(' ')} failed with exit code ${result.status ?? 'unknown'}`)
  error.exitCode = result.status ?? 1
  throw error
}

export function runOrExit(command, args, options = {}) {
  try {
    run(command, args, options)
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    process.exit(error?.exitCode ?? 1)
  }
}

export function runWithOutput(command, args, options = {}) {
  const result = run(command, args, {
    ...options,
    encoding: options.encoding ?? 'utf8',
    stdio: options.stdio ?? 'pipe',
  })
  return result.stdout
}

export function isDesktopArtifactName(name) {
  return /\.(?:deb|rpm|AppImage|dmg)$/i.test(name) || name.endsWith('.tar.gz')
}

export function isAuxiliaryArtifactName(name) {
  return name.startsWith('csv-anonymizer-repository-setup_')
}

export function isCurrentVersionDesktopArtifactName(name, version) {
  if (name.endsWith('.deb')) {
    return new RegExp(`^csv-anonymizer_${escapeRegExp(version)}_[A-Za-z0-9_]+\\.deb$`).test(name)
  }
  if (name.endsWith('.rpm')) {
    return new RegExp(`^csv-anonymizer-${escapeRegExp(version)}-[A-Za-z0-9_.+-]+\\.rpm$`).test(name)
  }
  if (name.endsWith('.AppImage')) {
    return new RegExp(`(^|[_ .-])${escapeRegExp(version)}([_ .-]|$)`).test(name)
  }
  if (name.endsWith('.dmg')) {
    return new RegExp(`^CSV\\.Anonymizer_${escapeRegExp(version)}_[A-Za-z0-9_]+\\.dmg$`).test(name)
  }
  if (name.endsWith('.tar.gz')) {
    return new RegExp(`^csv-anonymizer-${escapeRegExp(version)}-(?:linux|macos)-[A-Za-z0-9_+-]+(?:\\.app)?\\.tar\\.gz$`).test(name)
  }
  return false
}

export function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}
