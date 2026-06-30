#!/usr/bin/env node
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, copyFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { isCurrentVersionDesktopArtifactName, runOrExit } from './command-utils.mjs'

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const args = new Set(process.argv.slice(2))
const skipBuild = args.has('--skip-build')
const target = process.env.TAURI_TARGET ?? 'x86_64-unknown-linux-gnu'
const bundleRootCandidates = [
  path.join(repoRoot, 'target', target, 'release', 'bundle'),
  path.join(repoRoot, 'target', 'release', 'bundle'),
  path.join(repoRoot, 'src-tauri', 'target', target, 'release', 'bundle'),
  path.join(repoRoot, 'src-tauri', 'target', 'release', 'bundle'),
]
const artifactDir = path.join(repoRoot, 'dist', 'rust', 'artifacts')
const packageJson = JSON.parse(readFileSync(path.join(repoRoot, 'package.json'), 'utf8'))
const version = packageJson.version

function bundleDirectory() {
  for (const candidate of bundleRootCandidates) {
    if (existsSync(candidate)) return candidate
  }
  return bundleRootCandidates[0]
}

function collectArtifacts(root) {
  const candidates = [
    ['deb', '.deb'],
    ['rpm', '.rpm'],
    ['appimage', '.AppImage'],
  ]
  const artifacts = []

  for (const [directoryName, extension] of candidates) {
    const directory = path.join(root, directoryName)
    if (!existsSync(directory)) continue
    for (const entry of readdirSync(directory)) {
      if (entry.endsWith(extension)) {
        const artifact = path.join(directory, entry)
        if (!isCurrentVersionArtifact(entry)) {
          console.warn(`Skipping non-current Tauri artifact: ${path.relative(repoRoot, artifact)}`)
          continue
        }
        artifacts.push(artifact)
      }
    }
  }

  return artifacts
}

function isCurrentVersionArtifact(name) {
  return isCurrentVersionDesktopArtifactName(name, version)
}

if (!skipBuild) {
  if (process.env.CSV_ANONYMIZER_USE_PREBUILT_FRONTEND === '1') {
    runOrExit('bash', ['scripts/build_frontend_for_tauri.sh'], {
      cwd: repoRoot,
      env: { CSV_ANONYMIZER_USE_PREBUILT_FRONTEND: '1' },
    })
  }

  runOrExit('cargo', ['tauri', 'build', '--target', target], {
    cwd: path.join(repoRoot, 'src-tauri'),
  })
} else {
  runOrExit('bash', ['scripts/build_frontend_for_tauri.sh'], {
    cwd: repoRoot,
    env: { CSV_ANONYMIZER_USE_PREBUILT_FRONTEND: '1' },
  })
}

const root = bundleDirectory()
const artifacts = collectArtifacts(root)
if (artifacts.length === 0) {
  console.error(`No Tauri Linux artifacts found under ${root}.`)
  process.exit(1)
}

rmSync(artifactDir, { recursive: true, force: true })
mkdirSync(artifactDir, { recursive: true })
for (const artifact of artifacts) {
  copyFileSync(artifact, path.join(artifactDir, path.basename(artifact)))
}

console.log(`Copied ${artifacts.length} Tauri Linux artifact(s) to ${artifactDir}.`)
