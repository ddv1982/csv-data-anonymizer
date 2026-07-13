#!/usr/bin/env node
import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'
import { pathToFileURL } from 'node:url'

export function checkDocs(root) {
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

    for (const block of shellCodeBlocks(content)) {
      for (const match of block.matchAll(/\b(?:node|python3|bash)\s+(?:\.\/)?(scripts\/[A-Za-z0-9_./-]+)/g)) {
        const repositoryPath = match[1]
        const scriptsRoot = `${path.resolve(root, 'scripts')}${path.sep}`
        const commandPath = path.resolve(root, repositoryPath)
        const isScriptFile = commandPath.startsWith(scriptsRoot) && fs.existsSync(commandPath) && fs.statSync(commandPath).isFile()
        if (!isScriptFile) {
          errors.push(`${docPath}: documented repository command references missing file "${repositoryPath}"`)
        }
      }
    }
  }

  return { docs, errors }
}

function shellCodeBlocks(content) {
  return [...content.matchAll(/```(?:bash|sh|shell)\s*\n([\s\S]*?)```/g)].map((match) => match[1])
}

function main() {
  const { docs, errors } = checkDocs(process.cwd())
  if (errors.length > 0) {
    console.error('Documentation check failed:')
    for (const error of errors) {
      console.error(`- ${error}`)
    }
    process.exit(1)
  }

  console.log(`Documentation check passed for ${docs.length} markdown files.`)
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main()
}
