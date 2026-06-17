import { readdir, stat } from 'node:fs/promises'
import { join, relative } from 'node:path'

const artifactsDir = join(process.cwd(), 'dist', 'electrobun', 'artifacts')
const buildDir = join(process.cwd(), 'dist', 'electrobun', 'build')

const artifacts = await collectFiles(artifactsDir)
const builds = await collectFiles(buildDir)

if (builds.length === 0) {
  throw new Error(`No Electrobun build output found under ${relative(process.cwd(), buildDir)}.`)
}

if (artifacts.length === 0) {
  throw new Error(`No Electrobun artifacts found under ${relative(process.cwd(), artifactsDir)}.`)
}

const hasUpdateJson = artifacts.some((file) => file.endsWith('-update.json'))
const hasArchive = artifacts.some((file) => /\.(tar\.zst|tar\.gz|dmg|zip|exe)$/i.test(file))

if (!hasUpdateJson) {
  throw new Error('Electrobun artifacts are missing the platform update JSON.')
}

if (!hasArchive) {
  throw new Error('Electrobun artifacts are missing a distributable archive.')
}

console.log('Electrobun artifacts:')
for (const artifact of artifacts) {
  console.log(`- ${relative(process.cwd(), artifact)}`)
}

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true }).catch((error) => {
    if (error?.code === 'ENOENT') return []
    throw error
  })
  const files = []

  for (const entry of entries) {
    const path = join(directory, entry.name)
    if (entry.isDirectory()) {
      files.push(...await collectFiles(path))
      continue
    }

    if (entry.isFile() && (await stat(path)).size > 0) {
      files.push(path)
    }
  }

  return files.sort()
}
