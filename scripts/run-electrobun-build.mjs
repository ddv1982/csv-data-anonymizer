#!/usr/bin/env node
import { spawn } from 'node:child_process'

const hiddenNonMacBuildLines = new Set(['skipping codesign', 'skipping notarization'])
const hideNonMacBuildLines = process.platform !== 'darwin'
const child = spawn('bun', ['x', 'electrobun', 'build', ...process.argv.slice(2)], {
  stdio: ['inherit', 'pipe', 'pipe'],
  env: process.env
})

const stdoutFilter = createLineFilter(process.stdout)
const stderrFilter = createLineFilter(process.stderr)

child.stdout.on('data', stdoutFilter.write)
child.stderr.on('data', stderrFilter.write)
child.on('error', (error) => {
  stdoutFilter.flush()
  stderrFilter.flush()
  console.error(error.message)
  process.exitCode = 1
})
child.on('close', (code, signal) => {
  stdoutFilter.flush()
  stderrFilter.flush()
  if (signal) {
    process.kill(process.pid, signal)
    return
  }
  process.exitCode = code ?? 1
})

function createLineFilter(output) {
  let buffer = ''

  return {
    write(chunk) {
      buffer += chunk.toString()
      const lines = buffer.split(/\r?\n/)
      buffer = lines.pop() ?? ''
      for (const line of lines) writeLine(output, line, true)
    },
    flush() {
      if (!buffer) return
      writeLine(output, buffer, false)
      buffer = ''
    }
  }
}

function writeLine(output, line, appendNewline) {
  if (hideNonMacBuildLines && hiddenNonMacBuildLines.has(stripAnsi(line).trim())) return
  output.write(appendNewline ? `${line}\n` : line)
}

function stripAnsi(value) {
  return value.replace(/\u001B\[[0-9;]*m/g, '')
}
