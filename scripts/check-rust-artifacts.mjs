#!/usr/bin/env node
import { readdir, stat } from 'node:fs/promises'
import { join, relative } from 'node:path'

const args = new Set(process.argv.slice(2))
const platform = readOption('--platform') ?? process.platform
const requireRpm = args.has('--require-rpm')
const requireAppImage = args.has('--require-appimage')
const requireDmg = args.has('--require-dmg')
const artifactsDir = join(process.cwd(), 'dist', 'rust', 'artifacts')
const buildDir = join(process.cwd(), 'dist', 'rust', 'build')

const artifacts = await collectFiles(artifactsDir)
const builds = await collectFiles(buildDir)

if (builds.length === 0) {
  throw new Error(`No Rust build output found under ${relative(process.cwd(), buildDir)}.`)
}

if (artifacts.length === 0) {
  throw new Error(`No Rust artifacts found under ${relative(process.cwd(), artifactsDir)}.`)
}

const hasTarGz = artifacts.some((file) => file.endsWith('.tar.gz'))
if (!hasTarGz) {
  throw new Error('Rust artifacts are missing a portable .tar.gz archive.')
}

if (platform === 'linux') {
  requireArtifact((file) => file.endsWith('.deb'), 'Linux Rust artifacts are missing a .deb package.')
  if (requireRpm) {
    requireArtifact((file) => file.endsWith('.rpm'), 'Linux Rust artifacts are missing an .rpm package.')
  }
  if (requireAppImage) {
    requireArtifact((file) => file.endsWith('.AppImage'), 'Linux Rust artifacts are missing an AppImage.')
  }
}

if (platform === 'darwin' || platform === 'macos') {
  requireArtifact((file) => file.endsWith('.app.tar.gz'), 'macOS Rust artifacts are missing an app bundle archive.')
  if (requireDmg) {
    requireArtifact((file) => file.endsWith('.dmg'), 'macOS Rust artifacts are missing a .dmg installer.')
  }
}

console.log('Rust artifacts:')
for (const artifact of artifacts) {
  console.log(`- ${relative(process.cwd(), artifact)}`)
}

function readOption(name) {
  const index = process.argv.indexOf(name)
  if (index === -1) return undefined
  const value = process.argv[index + 1]
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`)
  }
  return value
}

function requireArtifact(predicate, message) {
  if (!artifacts.some(predicate)) {
    throw new Error(message)
  }
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
