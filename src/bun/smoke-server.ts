import { readFile } from 'node:fs/promises'
import { Utils } from 'electrobun/bun'
import type { CsvAnonymizerRpcHandlers } from './rpc'

type SmokeRunRequest = {
  filePath?: unknown
  outputPath?: unknown
  columns?: unknown
  seed?: unknown
  sampleRows?: unknown
  sampleCount?: unknown
  exitAfterRun?: unknown
}

type NormalizedSmokeRunRequest = {
  filePath: string
  outputPath: string
  columns: number[]
  seed: string
  sampleRows: number
  sampleCount: number
  exitAfterRun: boolean
}

export function startSmokeServer(handlers: CsvAnonymizerRpcHandlers): void {
  if (process.env.CSV_ANONYMIZER_SMOKE_SERVER !== '1') return

  const server = Bun.serve({
    hostname: '127.0.0.1',
    port: parsePort(process.env.CSV_ANONYMIZER_SMOKE_PORT),
    async fetch(request) {
      const url = new URL(request.url)

      try {
        if (request.method === 'GET' && url.pathname === '/health') {
          return jsonResponse({ health: await handlers.getHealth(undefined) })
        }

        if (request.method === 'POST' && url.pathname === '/run') {
          return jsonResponse(await runSmokeWorkflow(handlers, await request.json()))
        }

        if (request.method === 'POST' && url.pathname === '/quit') {
          setTimeout(() => Utils.quit(), 50).unref()
          return jsonResponse({ quitting: true })
        }

        return jsonResponse({ error: 'Not found' }, 404)
      } catch (error) {
        return jsonResponse(
          {
            error: error instanceof Error ? error.message : 'Unexpected smoke server error'
          },
          500
        )
      }
    }
  })

  console.log(`Smoke server started at ${server.url.origin}`)
}

async function runSmokeWorkflow(handlers: CsvAnonymizerRpcHandlers, rawBody: unknown): Promise<unknown> {
  const body = normalizeRunRequest(rawBody)
  const health = await handlers.getHealth(undefined)
  const headers = await handlers.getHeaders({ filePath: body.filePath, sampleRows: body.sampleRows })
  const preview = await handlers.getPreview({
    filePath: body.filePath,
    columns: body.columns,
    deterministic: true,
    seed: body.seed,
    sampleCount: body.sampleCount
  })
  const anonymized = await handlers.anonymizeFile({
    filePath: body.filePath,
    outputPath: body.outputPath,
    columns: body.columns,
    deterministic: true,
    seed: body.seed,
    force: true
  })
  const output = anonymized.success ? await readFile(body.outputPath, 'utf8') : null

  if (body.exitAfterRun) {
    setTimeout(() => Utils.quit(), 100).unref()
  }

  return {
    health,
    headers,
    preview,
    anonymized,
    output
  }
}

function normalizeRunRequest(rawBody: unknown): NormalizedSmokeRunRequest {
  if (!rawBody || typeof rawBody !== 'object') {
    throw new Error('Smoke request body must be a JSON object.')
  }

  const body = rawBody as SmokeRunRequest
  if (typeof body.filePath !== 'string' || body.filePath.trim().length === 0) {
    throw new Error('Smoke request requires a filePath string.')
  }

  if (typeof body.outputPath !== 'string' || body.outputPath.trim().length === 0) {
    throw new Error('Smoke request requires an outputPath string.')
  }

  return {
    filePath: body.filePath,
    outputPath: body.outputPath,
    columns: normalizeColumns(body.columns),
    seed: typeof body.seed === 'string' && body.seed.trim().length > 0 ? body.seed : 'electrobun-smoke',
    sampleRows: normalizePositiveInteger(body.sampleRows, 20),
    sampleCount: normalizePositiveInteger(body.sampleCount, 3),
    exitAfterRun: body.exitAfterRun === true
  }
}

function normalizeColumns(value: unknown): number[] {
  if (!Array.isArray(value) || value.length === 0) return [1]

  const columns = value.filter((column): column is number => Number.isInteger(column) && column >= 0)
  if (columns.length === 0) throw new Error('Smoke request columns must contain non-negative integers.')
  return columns
}

function normalizePositiveInteger(value: unknown, fallback: number): number {
  return Number.isInteger(value) && Number(value) > 0 ? Number(value) : fallback
}

function parsePort(value: string | undefined): number {
  if (!value) return 0
  const parsed = Number.parseInt(value, 10)
  return Number.isInteger(parsed) && parsed >= 0 ? parsed : 0
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      'content-type': 'application/json'
    }
  })
}
