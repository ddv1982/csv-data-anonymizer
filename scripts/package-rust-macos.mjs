#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  statSync,
  writeFileSync
} from 'node:fs'
import { basename, dirname, join, relative } from 'node:path'
import { tmpdir } from 'node:os'

const projectRoot = process.cwd()
const args = new Set(process.argv.slice(2))
const skipBuild = args.has('--skip-build')
const skipDmg = args.has('--skip-dmg')
const appOnly = args.has('--app-only')
const dmgOnly = args.has('--dmg-only')
const skipMissingTools = args.has('--skip-missing-tools')

const packageJson = JSON.parse(readFileSync(join(projectRoot, 'package.json'), 'utf8'))
const packageName = 'csv-anonymizer'
const binaryName = 'csv-anonymizer'
const appName = 'CSV Anonymizer'
const bundleIdentifier = 'io.github.ddv1982.csv-data-anonymizer'
const version = packageJson.version
const macArch = process.env.MACOS_ARCH || (process.arch === 'arm64' ? 'arm64' : 'x64')
const artifactArch = macArch === 'arm64' ? 'aarch64' : macArch
const artifactsDir = join(projectRoot, 'dist', 'rust', 'artifacts')
const buildDir = join(projectRoot, 'dist', 'rust', 'build')
const appDir = join(buildDir, `${appName}.app`)

mkdirSync(artifactsDir, { recursive: true })
mkdirSync(buildDir, { recursive: true })

if (!dmgOnly) {
  const binaryPath = findOrBuildBinary()
  createAppBundle(binaryPath)
  buildAppArchive()
}

if (!skipDmg && !appOnly) {
  buildDmg()
}

console.log('Rust macOS artifacts:')
for (const artifact of listArtifacts()) {
  console.log(`- ${relative(projectRoot, artifact)}`)
}

function findOrBuildBinary() {
  if (!skipBuild) {
    const cargoArgs = ['build', '--release', '-p', 'csv-anonymizer-app']
    if (process.env.CARGO_BUILD_TARGET) {
      cargoArgs.push('--target', process.env.CARGO_BUILD_TARGET)
    }
    run('cargo', cargoArgs)
  }

  const candidates = []
  if (process.env.CSV_ANONYMIZER_BINARY) {
    candidates.push(process.env.CSV_ANONYMIZER_BINARY)
  }
  if (process.env.CARGO_BUILD_TARGET) {
    candidates.push(join(projectRoot, 'target', process.env.CARGO_BUILD_TARGET, 'release', binaryName))
  }
  candidates.push(join(projectRoot, 'target', 'release', binaryName))

  const binary = candidates.find((candidate) => existsSync(candidate))
  if (!binary) {
    throw new Error(`Rust binary not found. Checked: ${candidates.map((candidate) => relative(projectRoot, candidate)).join(', ')}`)
  }
  return binary
}

function createAppBundle(binaryPath) {
  rmSync(appDir, { recursive: true, force: true })
  const contentsDir = join(appDir, 'Contents')
  const macosDir = join(contentsDir, 'MacOS')
  const resourcesDir = join(contentsDir, 'Resources')
  mkdirSync(macosDir, { recursive: true })
  mkdirSync(resourcesDir, { recursive: true })

  const installedBinary = join(macosDir, binaryName)
  copyFileSync(binaryPath, installedBinary)
  chmodSync(installedBinary, 0o755)
  writeFileSync(join(contentsDir, 'Info.plist'), infoPlist())
  writeFileSync(join(contentsDir, 'PkgInfo'), 'APPL????')
  createIcon(resourcesDir)
}

function createIcon(resourcesDir) {
  const iconset = join(projectRoot, 'build', 'macos', 'AppIcon.iconset')
  const output = join(resourcesDir, 'AppIcon.icns')
  const iconutil = resolveTool('iconutil')
  if (iconutil && existsSync(iconset)) {
    run(iconutil, ['-c', 'icns', iconset, '-o', output])
    return
  }

  const png = join(projectRoot, 'build', 'icons', '1024x1024.png')
  if (existsSync(png)) {
    copyFileSync(png, join(resourcesDir, 'AppIcon.png'))
    return
  }

  if (!skipMissingTools) {
    throw new Error('Could not create macOS icon: iconutil is missing and no fallback PNG exists.')
  }
}

function buildAppArchive() {
  const archivePath = join(artifactsDir, `${packageName}-${version}-macos-${artifactArch}.app.tar.gz`)
  rmSync(archivePath, { force: true })
  run('tar', ['-czf', archivePath, '-C', buildDir, basename(appDir)], {
    env: { ...process.env, COPYFILE_DISABLE: '1' }
  })
}

function buildDmg() {
  if (!existsSync(appDir)) {
    throw new Error(`App bundle not found at ${relative(projectRoot, appDir)}.`)
  }

  const hdiutil = resolveTool('hdiutil')
  if (!hdiutil) {
    if (skipMissingTools) {
      console.warn('Skipping DMG generation: hdiutil was not found.')
      return
    }
    throw new Error('DMG generation requires hdiutil.')
  }

  const dmgRoot = join(tmpdir(), `${packageName}-dmg-${Date.now()}`)
  const dmgPath = join(artifactsDir, `CSV.Anonymizer_${version}_${artifactArch}.dmg`)
  rmSync(dmgRoot, { recursive: true, force: true })
  rmSync(dmgPath, { force: true })
  mkdirSync(dmgRoot, { recursive: true })
  copyTree(appDir, join(dmgRoot, basename(appDir)))
  symlinkSync('/Applications', join(dmgRoot, 'Applications'))
  try {
    run(hdiutil, ['create', '-volname', appName, '-srcfolder', dmgRoot, '-ov', '-format', 'UDZO', dmgPath])
  } finally {
    rmSync(dmgRoot, { recursive: true, force: true })
  }
}

function infoPlist() {
  return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>${appName}</string>
  <key>CFBundleExecutable</key>
  <string>${binaryName}</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>CFBundleIdentifier</key>
  <string>${bundleIdentifier}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>${appName}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${version}</string>
  <key>CFBundleVersion</key>
  <string>${version}</string>
  <key>LSMinimumSystemVersion</key>
  <string>12.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSHumanReadableCopyright</key>
  <string>Copyright © 2026 Douwe de Vries</string>
</dict>
</plist>
`
}

function listArtifacts() {
  const candidates = [
    join(artifactsDir, `${packageName}-${version}-macos-${artifactArch}.app.tar.gz`),
    join(artifactsDir, `CSV.Anonymizer_${version}_${artifactArch}.dmg`)
  ]
  return candidates.filter((candidate) => existsSync(candidate))
}

function resolveTool(tool) {
  if (!tool) return ''
  if (tool.includes('/') && existsSync(tool)) return tool
  const result = spawnSync('sh', ['-c', `command -v ${shellQuote(tool)}`], { encoding: 'utf8' })
  return result.status === 0 ? result.stdout.trim() : ''
}

function run(command, runArgs, options = {}) {
  const result = spawnSync(command, runArgs, {
    cwd: options.cwd ?? projectRoot,
    env: options.env ?? process.env,
    stdio: options.stdio ?? 'inherit',
    encoding: 'utf8'
  })
  if (result.status !== 0) {
    throw new Error(`${command} ${runArgs.join(' ')} failed with exit code ${result.status ?? 'unknown'}`)
  }
}

function copyTree(source, target) {
  mkdirSync(target, { recursive: true })
  for (const entry of readdirSync(source, { withFileTypes: true })) {
    const sourcePath = join(source, entry.name)
    const targetPath = join(target, entry.name)
    if (entry.isDirectory()) {
      copyTree(sourcePath, targetPath)
    } else if (entry.isFile()) {
      copyFileSync(sourcePath, targetPath)
      chmodSync(targetPath, statSync(sourcePath).mode & 0o777)
    }
  }
}

function shellQuote(value) {
  return `'${value.replace(/'/g, "'\\''")}'`
}
