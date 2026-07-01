#!/usr/bin/env node
// Ensure frontend/dist exists so src-tauri/build.rs (which embeds the bundle)
// can run under cargo check/clippy/test without rebuilding the frontend on
// every local gate. Builds the frontend only when the bundle is missing.
import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { join } from 'node:path'

const projectRoot = process.cwd()
const indexHtml = join(projectRoot, 'frontend', 'dist', 'index.html')

if (existsSync(indexHtml)) {
  console.log('frontend/dist/index.html already exists; skipping frontend build.')
  process.exit(0)
}

console.log('frontend/dist/index.html is missing; building the frontend...')
const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm'
const result = spawnSync(npm, ['--prefix', 'frontend', 'run', 'build'], {
  cwd: projectRoot,
  stdio: 'inherit',
  shell: false
})

if (result.error) {
  console.error(`Failed to run npm: ${result.error.message}`)
  process.exit(1)
}
if (result.status !== 0) {
  console.error(`Frontend build failed with exit code ${result.status ?? 'unknown'}.`)
  process.exit(result.status ?? 1)
}
