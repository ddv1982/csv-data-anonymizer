#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { chmod, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const fingerprintVariable = 'CSV_ANONYMIZER_REPOSITORY_SETUP_SIGNING_KEY_FINGERPRINT'
const placeholder = '__CSV_ANONYMIZER_APT_SIGNING_KEY_FINGERPRINT__'
const sampleFingerprint = '0123456789ABCDEF0123456789ABCDEF01234567'
const overrideFingerprint = '89ABCDEF0123456789ABCDEF0123456789ABCDEF'
const normalizationLine =
  'expected_signing_fingerprint="$(printf \'%s\' "$expected_signing_fingerprint" | tr -d \'[:space:]\' | tr \'[:lower:]\' \'[:upper:]\')"'

const args = process.argv.slice(2)
const renderedInstallerPath = readOption('--rendered-installer')
const expectedFingerprint = normalizeFingerprint(readOption('--expected-fingerprint') ?? sampleFingerprint)

if (!isFingerprint(expectedFingerprint)) {
  throw new Error(`Expected signing fingerprint must be a 40-character hex fingerprint, got ${expectedFingerprint}`)
}

const template = await readFile('scripts/install-apt-repo.sh', 'utf8')
await validateTemplate(template)

const renderedInstaller = renderedInstallerPath
  ? await readFile(renderedInstallerPath, 'utf8')
  : template.replaceAll(placeholder, expectedFingerprint)

await validateRenderedInstaller(renderedInstaller, expectedFingerprint)
console.log('APT installer fingerprint check passed.')

function readOption(name) {
  const index = args.indexOf(name)
  if (index === -1) {
    return undefined
  }

  const value = args[index + 1]
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`)
  }

  return value
}

async function validateTemplate(script) {
  if (!script.includes(placeholder)) {
    throw new Error(`scripts/install-apt-repo.sh must contain ${placeholder} for release-time rendering`)
  }

  const defaultFingerprint = await probeEffectiveFingerprint(script, {})
  if (defaultFingerprint !== '') {
    throw new Error('The template installer must not trust an unresolved placeholder fingerprint by default.')
  }

  const envFingerprint = await probeEffectiveFingerprint(script, {
    [fingerprintVariable]: ` ${overrideFingerprint.toLowerCase()} `
  })
  if (envFingerprint !== overrideFingerprint) {
    throw new Error(`The template installer did not preserve an explicit ${fingerprintVariable} override.`)
  }
}

async function validateRenderedInstaller(script, expected) {
  if (script.includes(placeholder)) {
    throw new Error(`Rendered APT installer still contains ${placeholder}`)
  }

  const defaultFingerprint = await probeEffectiveFingerprint(script, {})
  if (defaultFingerprint !== expected) {
    throw new Error(`Rendered APT installer default fingerprint resolved to ${defaultFingerprint || '<empty>'}, expected ${expected}`)
  }

  const envFingerprint = await probeEffectiveFingerprint(script, {
    [fingerprintVariable]: ` ${overrideFingerprint.toLowerCase()} `
  })
  if (envFingerprint !== overrideFingerprint) {
    throw new Error(`Rendered APT installer did not preserve an explicit ${fingerprintVariable} override.`)
  }
}

async function probeEffectiveFingerprint(script, envOverrides) {
  if (!script.includes(normalizationLine)) {
    throw new Error('Could not find the installer fingerprint normalization line to instrument.')
  }

  const tmpDir = await mkdtemp(join(tmpdir(), 'csv-installer-check-'))
  const scriptPath = join(tmpDir, 'install-apt-repo.sh')
  const instrumentedScript = script.replace(
    normalizationLine,
    `${normalizationLine}\nprintf 'CSV_ANONYMIZER_EFFECTIVE_FINGERPRINT=%s\\n' "$expected_signing_fingerprint"\nexit 0`
  )

  try {
    await writeFile(scriptPath, instrumentedScript, 'utf8')
    await chmod(scriptPath, 0o755)

    const env = { ...process.env }
    delete env[fingerprintVariable]
    Object.assign(env, envOverrides)

    const result = spawnSync('sh', [scriptPath], {
      encoding: 'utf8',
      env
    })

    if (result.status !== 0) {
      throw new Error(`Instrumented installer exited with ${result.status ?? 'null'}: ${result.stderr || result.stdout}`)
    }

    const match = result.stdout.match(/^CSV_ANONYMIZER_EFFECTIVE_FINGERPRINT=(.*)$/m)
    if (!match) {
      throw new Error(`Instrumented installer did not print the effective fingerprint. Output: ${result.stdout}`)
    }

    return match[1]
  } finally {
    await rm(tmpDir, { recursive: true, force: true })
  }
}

function normalizeFingerprint(value) {
  return value.replace(/\s+/g, '').toUpperCase()
}

function isFingerprint(value) {
  return /^[A-F0-9]{40}$/.test(value)
}
