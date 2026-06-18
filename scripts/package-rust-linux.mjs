#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync
} from 'node:fs'
import { basename, dirname, join, relative } from 'node:path'
import { tmpdir } from 'node:os'

const projectRoot = process.cwd()
const args = new Set(process.argv.slice(2))
const skipBuild = args.has('--skip-build')
const skipRpm = args.has('--skip-rpm')
const skipAppImage = args.has('--skip-appimage')
const skipMissingTools = args.has('--skip-missing-tools')

const packageJson = JSON.parse(readFileSync(join(projectRoot, 'package.json'), 'utf8'))
const packageName = 'csv-anonymizer'
const binaryName = 'csv-anonymizer'
const appName = 'CSV Anonymizer'
const componentId = 'io.github.ddv1982.csv-data-anonymizer'
const desktopId = 'csv-anonymizer.desktop'
const version = packageJson.version
const description = packageJson.description ?? 'Desktop CSV anonymizer for local-first anonymization workflows.'
const maintainer = packageJson.author ?? 'Douwe de Vries <douwe.de.vries.82@gmail.com>'
const homepage = packageJson.homepage ?? 'https://github.com/ddv1982/csv-data-anonymizer'
const linuxArch = process.env.LINUX_ARCH || (process.arch === 'arm64' ? 'arm64' : 'x64')
const debArch = process.env.DEB_ARCH || (linuxArch === 'arm64' ? 'arm64' : 'amd64')
const rpmArch = process.env.RPM_ARCH || (linuxArch === 'arm64' ? 'aarch64' : 'x86_64')
const appImageArch = process.env.APPIMAGE_ARCH || (linuxArch === 'arm64' ? 'aarch64' : 'x86_64')
const artifactsDir = join(projectRoot, 'dist', 'rust', 'artifacts')
const buildDir = join(projectRoot, 'dist', 'rust', 'build')
const workDir = join(projectRoot, 'dist', 'rust', 'linux-packaging')
const binaryPath = findOrBuildBinary()

rmSync(workDir, { recursive: true, force: true })
mkdirSync(artifactsDir, { recursive: true })
mkdirSync(workDir, { recursive: true })

buildPortableArchive()

const debLayout = join(workDir, 'deb-root')
createPackageLayout(debLayout, { rpm: false, appImage: false })
const debPath = join(artifactsDir, `${packageName}_${version}_${debArch}.deb`)
buildDeb(debLayout, debPath)

if (!skipRpm) {
  runToolOrSkip('rpmbuild', 'RPM package generation', () => {
    const rpmLayout = join(workDir, 'rpm-root')
    createPackageLayout(rpmLayout, { rpm: true, appImage: false })
    buildRpm(rpmLayout)
  })
}

if (!skipAppImage) {
  runToolOrSkip(process.env.APPIMAGETOOL || 'appimagetool', 'AppImage generation', (tool) => {
    const appDir = join(workDir, 'CSVAnonymizer.AppDir')
    createPackageLayout(appDir, { rpm: true, appImage: true })
    buildAppImage(appDir, tool)
  })
}

console.log('Rust Linux artifacts:')
for (const artifact of readdirSync(artifactsDir).filter((name) => /\.(deb|rpm|AppImage|tar\.gz)$/i.test(name)).sort()) {
  console.log(`- ${relative(projectRoot, join(artifactsDir, artifact))}`)
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

function buildPortableArchive() {
  const portableRoot = join(buildDir, `${packageName}-${version}-linux-${linuxArch}`)
  rmSync(portableRoot, { recursive: true, force: true })
  mkdirSync(portableRoot, { recursive: true })
  copyFileSync(binaryPath, join(portableRoot, binaryName))
  chmodSync(join(portableRoot, binaryName), 0o755)
  copyFileSync(join(projectRoot, 'README.md'), join(portableRoot, 'README.md'))
  copyFileSync(join(projectRoot, 'LICENSE'), join(portableRoot, 'LICENSE'))
  const archivePath = join(artifactsDir, `${packageName}-${version}-linux-${linuxArch}.tar.gz`)
  rmSync(archivePath, { force: true })
  createTarGz(dirname(portableRoot), archivePath, basename(portableRoot))
}

function createPackageLayout(root, options) {
  rmSync(root, { recursive: true, force: true })
  mkdirSync(root, { recursive: true })

  const installedBinary = join(root, 'usr', 'bin', binaryName)
  mkdirSync(dirname(installedBinary), { recursive: true })
  copyFileSync(binaryPath, installedBinary)
  chmodSync(installedBinary, 0o755)

  const desktop = desktopEntry(options.appImage)
  writeText(join(root, 'usr', 'share', 'applications', desktopId), desktop)
  writeText(join(root, 'usr', 'share', 'metainfo', `${componentId}.metainfo.xml`), appStreamMetadata())
  copyIcons(root)
  writeText(join(root, 'usr', 'share', 'doc', packageName, 'copyright'), readFileSync(copyrightPath(), 'utf8'))
  if (options.rpm) {
    writeText(join(root, 'usr', 'share', 'licenses', packageName, 'LICENSE'), readFileSync(join(projectRoot, 'LICENSE'), 'utf8'))
  }

  if (options.appImage) {
    writeExecutable(
      join(root, 'AppRun'),
      `#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/${binaryName}" "$@"
`
    )
    writeText(join(root, desktopId), desktop)
    const rootIcon = join(root, `${packageName}.png`)
    copyFileSync(iconPath(), rootIcon)
    copyFileSync(rootIcon, join(root, '.DirIcon'))
  }
}

function desktopEntry(appImage) {
  return `[Desktop Entry]
Type=Application
Name=${appName}
Comment=${description}
Exec=${appImage ? 'AppRun' : binaryName} %F
Icon=${packageName}
Terminal=false
Categories=Utility;
MimeType=text/csv;text/plain;
Keywords=CSV;Privacy;Anonymization;Data;
StartupWMClass=${appName}
`
}

function appStreamMetadata() {
  return readFileSync(join(projectRoot, 'build', 'linux', `${componentId}.metainfo.xml`), 'utf8')
}

function copyrightPath() {
  return join(projectRoot, 'build', 'linux', 'debian', 'copyright')
}

function iconPath() {
  const preferred = join(projectRoot, 'build', 'icons', '1024x1024.png')
  if (existsSync(preferred)) return preferred
  const icon = readdirSync(join(projectRoot, 'build', 'icons')).find((name) => name.endsWith('.png'))
  if (!icon) throw new Error('No PNG icons found under build/icons.')
  return join(projectRoot, 'build', 'icons', icon)
}

function copyIcons(root) {
  const iconRoot = join(projectRoot, 'build', 'icons')
  for (const file of readdirSync(iconRoot)) {
    if (!file.endsWith('.png')) continue
    const size = basename(file, '.png')
    if (!/^\d+x\d+$/.test(size)) continue
    const target = join(root, 'usr', 'share', 'icons', 'hicolor', size, 'apps', `${packageName}.png`)
    mkdirSync(dirname(target), { recursive: true })
    copyFileSync(join(iconRoot, file), target)
  }
}

function buildDeb(layoutRoot, outputPath) {
  const temp = mkdtempSync(join(tmpdir(), 'csv-anonymizer-deb-'))
  try {
    const controlRoot = join(temp, 'control')
    mkdirSync(controlRoot, { recursive: true })
    const installedSize = Math.ceil(totalFileSize(layoutRoot) / 1024)
    writeText(
      join(controlRoot, 'control'),
      `Package: ${packageName}
Version: ${version}
Architecture: ${debArch}
Maintainer: ${maintainer}
Installed-Size: ${installedSize}
Depends: libgtk-3-0, libasound2t64 | libasound2, libxcb-render0, libxcb-shape0, libxcb-xfixes0, libxkbcommon0, libwayland-client0, libssl3 | libssl1.1
Section: utils
Priority: optional
Homepage: ${homepage}
Description: ${description}
 CSV Anonymizer detects sensitive CSV columns, previews deterministic transformations,
 and writes anonymized output without sending data off the device.
`
    )
    writeText(join(controlRoot, 'md5sums'), md5Sums(layoutRoot))

    const controlTar = join(temp, 'control.tar.gz')
    const dataTar = join(temp, 'data.tar.gz')
    createTarGz(controlRoot, controlTar, '.')
    createTarGz(layoutRoot, dataTar, '.')
    writeDebArchive(outputPath, [
      ['debian-binary', Buffer.from('2.0\n')],
      ['control.tar.gz', readFileSync(controlTar)],
      ['data.tar.gz', readFileSync(dataTar)]
    ])
  } finally {
    rmSync(temp, { recursive: true, force: true })
  }
}

function buildRpm(layoutRoot) {
  const rpmRoot = join(workDir, 'rpmbuild')
  const specPath = join(rpmRoot, 'SPECS', `${packageName}.spec`)
  const payloadPath = join(rpmRoot, 'SOURCES', 'payload')
  rmSync(rpmRoot, { recursive: true, force: true })
  mkdirSync(dirname(specPath), { recursive: true })
  mkdirSync(payloadPath, { recursive: true })
  for (const dir of ['BUILD', 'BUILDROOT', 'RPMS', 'SRPMS']) {
    mkdirSync(join(rpmRoot, dir), { recursive: true })
  }
  copyTree(layoutRoot, payloadPath)
  writeText(
    specPath,
    `Name: ${packageName}
Version: ${version}
Release: 1
Summary: Local-first CSV anonymization desktop app
License: MIT
URL: ${homepage}
Packager: ${maintainer}
AutoReqProv: yes

%description
${description}

%install
rm -rf "%{buildroot}"
mkdir -p "%{buildroot}"
cp -a "%{_sourcedir}/payload/." "%{buildroot}/"

%files
%defattr(-,root,root,-)
/usr/bin/${binaryName}
/usr/share/applications/${desktopId}
/usr/share/metainfo/${componentId}.metainfo.xml
/usr/share/icons/hicolor
/usr/share/doc/${packageName}/copyright
%license /usr/share/licenses/${packageName}/LICENSE

%changelog
* Thu Jun 18 2026 ${maintainer} - ${version}-1
- Package native Rust Linux build.
`
  )
  run('rpmbuild', ['--define', `_topdir ${rpmRoot}`, '--target', rpmArch, '-bb', specPath])
  const rpmDir = join(rpmRoot, 'RPMS', rpmArch)
  const rpm = readdirSync(rpmDir).find((name) => name.endsWith('.rpm'))
  if (!rpm) throw new Error(`rpmbuild did not produce an RPM under ${relative(projectRoot, rpmDir)}.`)
  copyFileSync(join(rpmDir, rpm), join(artifactsDir, rpm))
}

function buildAppImage(appDir, appimagetool) {
  const outputPath = join(artifactsDir, `CSVAnonymizer-${version}-${appImageArch}.AppImage`)
  rmSync(outputPath, { force: true })
  run(appimagetool, [appDir, outputPath], {
    env: {
      ...process.env,
      ARCH: appImageArch,
      APPIMAGE_EXTRACT_AND_RUN: process.env.APPIMAGE_EXTRACT_AND_RUN || '1'
    }
  })
  chmodSync(outputPath, 0o755)
}

function runToolOrSkip(tool, label, callback) {
  const resolved = resolveTool(tool)
  if (!resolved) {
    if (skipMissingTools) {
      console.warn(`Skipping ${label}: ${tool} was not found.`)
      return
    }
    throw new Error(`${label} requires missing tool: ${tool}`)
  }
  callback(resolved)
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

function createTarGz(sourceDir, outputPath, entry) {
  mkdirSync(dirname(outputPath), { recursive: true })
  run('tar', ['-czf', outputPath, '-C', sourceDir, entry], {
    env: {
      ...process.env,
      COPYFILE_DISABLE: '1'
    }
  })
}

function writeDebArchive(outputPath, members) {
  const buffers = [Buffer.from('!<arch>\n')]
  for (const [name, data] of members) {
    buffers.push(arMember(name, data))
  }
  writeFileSync(outputPath, Buffer.concat(buffers))
}

function arMember(name, data) {
  const encodedName = Buffer.from(`${name}/`)
  if (encodedName.length > 16) throw new Error(`ar member name is too long: ${name}`)
  const header = Buffer.concat([
    padBuffer(encodedName, 16),
    padBuffer(Buffer.from('0'), 12),
    padBuffer(Buffer.from('0'), 6),
    padBuffer(Buffer.from('0'), 6),
    padBuffer(Buffer.from('100644'), 8),
    padBuffer(Buffer.from(String(data.length)), 10),
    Buffer.from('`\n')
  ])
  return Buffer.concat([header, data, data.length % 2 === 1 ? Buffer.from('\n') : Buffer.alloc(0)])
}

function padBuffer(buffer, length) {
  if (buffer.length > length) throw new Error(`buffer exceeds fixed field length ${length}`)
  return Buffer.concat([buffer, Buffer.alloc(length - buffer.length, ' ')])
}

function md5Sums(root) {
  return collectFiles(root)
    .map((file) => `${hashFile(file, 'md5')}  ${relative(root, file)}\n`)
    .join('')
}

function totalFileSize(root) {
  return collectFiles(root).reduce((total, file) => total + statSync(file).size, 0)
}

function collectFiles(root) {
  const files = []
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = join(root, entry.name)
    if (entry.isDirectory()) {
      files.push(...collectFiles(path))
    } else if (entry.isFile()) {
      files.push(path)
    }
  }
  return files.sort()
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

function hashFile(file, algorithm) {
  return createHash(algorithm).update(readFileSync(file)).digest('hex')
}

function writeText(path, contents) {
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, contents)
}

function writeExecutable(path, contents) {
  writeText(path, contents)
  chmodSync(path, 0o755)
}

function shellQuote(value) {
  return `'${value.replace(/'/g, "'\\''")}'`
}
