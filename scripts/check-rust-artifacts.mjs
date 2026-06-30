#!/usr/bin/env node
import { readFile } from 'node:fs/promises'
import { basename, join, relative } from 'node:path'
import {
  isAuxiliaryArtifactName,
  isCurrentVersionDesktopArtifactName,
  isDesktopArtifactName,
  readOption,
} from './command-utils.mjs'

const args = new Set(process.argv.slice(2))
const optionArgs = process.argv.slice(2)
const platform = readOption(optionArgs, '--platform') ?? process.platform
const requireRpm = args.has('--require-rpm')
const requireAppImage = args.has('--require-appimage')
const requireDmg = args.has('--require-dmg')
const requireTarGz = args.has('--require-tar-gz')
const artifactsDir = join(process.cwd(), 'dist', 'rust', 'artifacts')
const buildDir = join(process.cwd(), 'dist', 'rust', 'build')
const packageJson = JSON.parse(await readFile(join(process.cwd(), 'package.json'), 'utf8'))
const version = packageJson.version

const artifacts = await collectDesktopArtifacts(artifactsDir)
const builds = await collectFiles(buildDir)

if (artifacts.length === 0) {
  throw new Error(`No desktop artifacts found under ${relative(process.cwd(), artifactsDir)}.`)
}

const invalidArtifacts = artifacts.filter((file) => !isCurrentVersionArtifact(file))
if (invalidArtifacts.length > 0) {
  throw new Error(
    [
      `Desktop artifacts must match current package version ${version}.`,
      ...invalidArtifacts.map((file) => `- ${relative(process.cwd(), file)}`)
    ].join('\n')
  )
}

const hasTarGz = artifacts.some((file) => file.endsWith('.tar.gz'))
if (requireTarGz && !hasTarGz) {
  throw new Error('Desktop artifacts are missing a portable .tar.gz archive.')
}

if (platform === 'linux') {
  requireArtifact((file) => file.endsWith('.deb'), 'Linux desktop artifacts are missing a .deb package.')
  if (requireRpm) {
    requireArtifact((file) => file.endsWith('.rpm'), 'Linux desktop artifacts are missing an .rpm package.')
  }
  if (requireAppImage) {
    requireArtifact((file) => file.endsWith('.AppImage'), 'Linux desktop artifacts are missing an AppImage.')
  }
}

if (platform === 'darwin' || platform === 'macos') {
  if (builds.length === 0) {
    throw new Error(`No desktop build output found under ${relative(process.cwd(), buildDir)}.`)
  }
  if (requireDmg) {
    requireArtifact((file) => file.endsWith('.dmg'), 'macOS desktop artifacts are missing a .dmg installer.')
  }
}

console.log('Desktop artifacts:')
for (const artifact of artifacts) {
  console.log(`- ${relative(process.cwd(), artifact)}`)
}

function requireArtifact(predicate, message) {
  if (!artifacts.some(predicate)) {
    throw new Error(message)
  }
}

async function collectDesktopArtifacts(directory) {
  const files = await collectFiles(directory)
  return files.filter((file) => {
    const name = basename(file)
    return isDesktopArtifactName(name) && !isAuxiliaryArtifactName(name)
  })
}

async function collectFiles(directory) {
  const { readdir, stat } = await import('node:fs/promises')
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

function isCurrentVersionArtifact(file) {
  return isCurrentVersionDesktopArtifactName(basename(file), version)
}
