#!/usr/bin/env node
import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'

const root = process.cwd()
const packageJson = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'))
const scripts = new Set(Object.keys(packageJson.scripts ?? {}))
const docs = [
  'README.md',
  ...fs
    .readdirSync(path.join(root, 'docs'))
    .filter((fileName) => fileName.endsWith('.md'))
    .map((fileName) => path.join('docs', fileName)),
]

const errors = []

for (const docPath of docs) {
  const content = fs.readFileSync(path.join(root, docPath), 'utf8')
  for (const match of content.matchAll(/\bnpm run ([A-Za-z0-9:_-]+)/g)) {
    const scriptName = match[1]
    if (!scripts.has(scriptName)) {
      errors.push(`${docPath}: documented npm script "${scriptName}" is not defined in package.json`)
    }
  }
}

if (errors.length > 0) {
  console.error('Documentation check failed:')
  for (const error of errors) {
    console.error(`- ${error}`)
  }
  process.exit(1)
}

console.log(`Documentation check passed for ${docs.length} markdown files.`)
