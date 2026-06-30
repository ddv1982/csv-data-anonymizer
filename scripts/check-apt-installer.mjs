#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'
import { readOption } from './command-utils.mjs'

const fingerprintVariable = 'CSV_ANONYMIZER_REPOSITORY_SETUP_SIGNING_KEY_FINGERPRINT'
const placeholder = '__CSV_ANONYMIZER_APT_SIGNING_KEY_FINGERPRINT__'
const sampleFingerprint = '0123456789ABCDEF0123456789ABCDEF01234567'
const overrideFingerprint = '89ABCDEF0123456789ABCDEF0123456789ABCDEF'
const normalizationLine =
  'expected_signing_fingerprint="$(printf \'%s\' "$expected_signing_fingerprint" | tr -d \'[:space:]\' | tr \'[:lower:]\' \'[:upper:]\')"'

const args = process.argv.slice(2)
const renderedInstallerPath = readOption(args, '--rendered-installer')
const expectedFingerprint = normalizeFingerprint(readOption(args, '--expected-fingerprint') ?? sampleFingerprint)

if (!isFingerprint(expectedFingerprint)) {
  throw new Error(`Expected signing fingerprint must be a 40-character hex fingerprint, got ${expectedFingerprint}`)
}

const template = await readFile('scripts/install-apt-repo.sh', 'utf8')
await validateTemplate(template)

const renderedInstaller = renderedInstallerPath
  ? await readFile(renderedInstallerPath, 'utf8')
  : template.replaceAll(placeholder, expectedFingerprint)

await validateRenderedInstaller(renderedInstaller, expectedFingerprint)
await validateAptInstallStaging(renderedInstaller)
console.log('APT installer check passed.')

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

async function validateAptInstallStaging(script) {
  const tmpDir = await mkdtemp(join(tmpdir(), 'csv-installer-apt-check-'))
  const fakeBinDir = join(tmpDir, 'bin')
  const dummyDebPath = join(tmpDir, 'repository-setup.deb')
  const scriptPath = join(tmpDir, 'install-apt-repo.sh')
  const sudoPath = join(fakeBinDir, 'sudo')
  const curlPath = join(fakeBinDir, 'curl')
  const dummyDeb = 'dummy repository setup package\n'
  const expectedSha256 = createHash('sha256').update(dummyDeb).digest('hex')

  try {
    await mkdir(fakeBinDir)
    await writeFile(dummyDebPath, dummyDeb, 'utf8')
    await writeFile(scriptPath, script, 'utf8')
    await chmod(scriptPath, 0o755)

    await writeFile(
      sudoPath,
      `#!/bin/sh
set -eu

mode_for() {
  if stat -c %a "$1" >/dev/null 2>&1; then
    stat -c %a "$1"
  else
    stat -f %Lp "$1"
  fi
}

if [ "$#" -ne 4 ] || [ "$1" != "apt" ] || [ "$2" != "install" ] || [ "$3" != "-y" ]; then
  echo "Unexpected sudo invocation: $*" >&2
  exit 41
fi

deb="$4"
dir="$(dirname "$deb")"
dir_mode="$(mode_for "$dir")"
file_mode="$(mode_for "$deb")"

printf 'CSV_ANONYMIZER_APT_INSTALL_DEB=%s\\n' "$deb"
printf 'CSV_ANONYMIZER_APT_INSTALL_DIR_MODE=%s\\n' "$dir_mode"
printf 'CSV_ANONYMIZER_APT_INSTALL_FILE_MODE=%s\\n' "$file_mode"

case "$deb" in
  */csv-anonymizer-repository-install.*/*)
    ;;
  *)
    echo "APT install did not use the public installer staging directory: $deb" >&2
    exit 42
    ;;
esac

if [ "$dir_mode" != "755" ]; then
  echo "APT installer staging directory mode is $dir_mode, expected 755." >&2
  exit 43
fi

if [ "$file_mode" != "644" ]; then
  echo "APT installer package mode is $file_mode, expected 644." >&2
  exit 44
fi
`,
      'utf8'
    )
    await chmod(sudoPath, 0o755)

    await writeFile(
      curlPath,
      `#!/bin/sh
set -eu

output=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -fsSLo)
      output="$2"
      shift 2
      ;;
    file://*)
      url="$1"
      shift
      ;;
    *)
      shift
      ;;
  esac
done

if [ -z "$output" ] || [ -z "$url" ]; then
  echo "Unexpected curl invocation" >&2
  exit 51
fi

cp "\${url#file://}" "$output"
`,
      'utf8'
    )
    await chmod(curlPath, 0o755)

    const result = spawnSync('sh', [scriptPath], {
      encoding: 'utf8',
      env: {
        ...process.env,
        PATH: `${fakeBinDir}:${process.env.PATH ?? ''}`,
        TMPDIR: tmpDir,
        CSV_ANONYMIZER_REPOSITORY_SETUP_URL: pathToFileURL(dummyDebPath).href,
        CSV_ANONYMIZER_REPOSITORY_SETUP_SHA256: expectedSha256
      }
    })

    if (result.status !== 0) {
      throw new Error(`Installer APT staging probe exited with ${result.status ?? 'null'}:\n${result.stdout}${result.stderr}`)
    }

    for (const expectedLine of [
      'CSV_ANONYMIZER_APT_INSTALL_DIR_MODE=755',
      'CSV_ANONYMIZER_APT_INSTALL_FILE_MODE=644'
    ]) {
      if (!result.stdout.includes(expectedLine)) {
        throw new Error(`Installer APT staging probe did not print ${expectedLine}. Output:\n${result.stdout}`)
      }
    }
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
