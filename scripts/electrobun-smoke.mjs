import { spawn } from 'node:child_process'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const startupTimeoutMs = 30000
const smokeServerPattern = /Smoke server started at (http:\/\/127\.0\.0\.1:\d+)/

const tempDir = await mkdtemp(join(tmpdir(), 'csv-anonymizer-electrobun-smoke-'))
const inputPath = join(tempDir, 'input.csv')
const outputPath = join(tempDir, 'output.csv')

await writeFile(inputPath, 'id,email\n1,alice@example.com\n2,bob@example.com\n', 'utf8')

const child = spawn('pnpm', ['exec', 'electrobun', 'run'], {
  detached: process.platform !== 'win32',
  stdio: ['ignore', 'pipe', 'pipe'],
  env: {
    ...process.env,
    CSV_ANONYMIZER_SMOKE_SERVER: '1',
    CSV_ANONYMIZER_SMOKE_PORT: '0'
  }
})

let output = ''
let settled = false
let smokeStarted = false

const startupTimer = setTimeout(() => {
  void fail(new Error(`Electrobun app did not report smoke server startup within ${startupTimeoutMs}ms.`))
}, startupTimeoutMs)

child.stdout.on('data', (chunk) => handleOutput(chunk))
child.stderr.on('data', (chunk) => handleOutput(chunk))

child.on('error', (error) => {
  void fail(error)
})

child.on('exit', (code, signal) => {
  clearTimeout(startupTimer)
  if (settled) return

  void fail(new Error(`Electrobun app exited before smoke completed. code=${code ?? 'null'} signal=${signal ?? 'null'}`))
})

function handleOutput(chunk) {
  output += chunk.toString()
  if (smokeStarted) return

  const match = output.match(smokeServerPattern)
  if (!match?.[1]) return

  smokeStarted = true
  clearTimeout(startupTimer)
  void runSmoke(match[1]).catch((error) => {
    void fail(error)
  })
}

async function runSmoke(origin) {
  const response = await fetch(`${origin}/run`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json'
    },
    body: JSON.stringify({
      filePath: inputPath,
      outputPath,
      columns: [1],
      seed: 'electrobun-smoke',
      sampleRows: 10,
      sampleCount: 2,
      exitAfterRun: true
    })
  })

  if (!response.ok) {
    throw new Error(`Electrobun smoke endpoint returned ${response.status}: ${await response.text()}`)
  }

  const result = await response.json()
  assertApiSuccess(result.health, 'health')
  assertApiSuccess(result.headers, 'headers')
  assertApiSuccess(result.preview, 'preview')
  assertApiSuccess(result.anonymized, 'anonymize')

  const outputContents = await readFile(outputPath, 'utf8')
  if (!outputContents.includes('@example.com')) {
    throw new Error('Electrobun smoke output did not contain anonymized email domain.')
  }

  settled = true
  shutdown()
  await cleanup()
  console.log('Electrobun smoke passed.')
}

function assertApiSuccess(value, label) {
  if (!value?.success) {
    throw new Error(`Electrobun smoke ${label} call failed: ${JSON.stringify(value)}`)
  }
}

async function fail(error) {
  if (settled) return
  settled = true
  clearTimeout(startupTimer)
  shutdown()
  console.error(error.message)
  if (output.trim()) console.error(output.trim())
  await cleanup()
  process.exitCode = 1
}

async function cleanup() {
  await rm(tempDir, { recursive: true, force: true })
}

function shutdown() {
  if (child.killed) return

  if (process.platform !== 'win32' && child.pid) {
    try {
      process.kill(-child.pid, 'SIGINT')
    } catch {
      child.kill('SIGINT')
    }
    setTimeout(() => {
      try {
        process.kill(-child.pid, 'SIGTERM')
      } catch {
        // Process group already exited.
      }
    }, 1000).unref()
    return
  }

  child.kill('SIGINT')
  setTimeout(() => child.kill('SIGTERM'), 1000).unref()
}
