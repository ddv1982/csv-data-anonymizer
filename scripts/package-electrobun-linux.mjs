#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  chmodSync,
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync
} from 'node:fs'
import { basename, dirname, join, relative, resolve } from 'node:path'
import { tmpdir } from 'node:os'

const projectRoot = process.cwd()
const args = new Set(process.argv.slice(2))
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
const buildEnv = process.env.ELECTROBUN_BUILD_ENV || 'stable'
const linuxArch = process.env.ELECTROBUN_ARCH || (process.arch === 'arm64' ? 'arm64' : 'x64')
const debArch = process.env.DEB_ARCH || (linuxArch === 'arm64' ? 'arm64' : 'amd64')
const rpmArch = process.env.RPM_ARCH || (linuxArch === 'arm64' ? 'aarch64' : 'x86_64')
const appImageArch = process.env.APPIMAGE_ARCH || (linuxArch === 'arm64' ? 'aarch64' : 'x86_64')
const artifactsDir = join(projectRoot, 'dist', 'electrobun', 'artifacts')
const packagesDir = artifactsDir
const workDir = join(projectRoot, 'dist', 'electrobun', 'linux-packaging')

const bundleDir = findLinuxBundle()
rmSync(workDir, { recursive: true, force: true })
mkdirSync(packagesDir, { recursive: true })
mkdirSync(workDir, { recursive: true })

const debLayout = join(workDir, 'deb-root')
createPackageLayout(debLayout, { rpm: false, appImage: false })
const debPath = join(packagesDir, `${packageName}_${version}_${debArch}.deb`)
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

console.log('Linux package-manager artifacts:')
for (const artifact of readdirSync(packagesDir).filter((name) => /\.(deb|rpm|AppImage)$/i.test(name)).sort()) {
  console.log(`- ${relative(projectRoot, join(packagesDir, artifact))}`)
}

function findLinuxBundle() {
  const buildRoot = join(projectRoot, 'dist', 'electrobun', 'build')
  const expectedBuildDir = join(buildRoot, `${buildEnv}-linux-${linuxArch}`)
  const buildDirs = existsSync(expectedBuildDir)
    ? [expectedBuildDir]
    : existsSync(buildRoot)
      ? readdirSync(buildRoot)
          .filter((name) => name.startsWith(`${buildEnv}-linux-`))
          .map((name) => join(buildRoot, name))
      : []

  for (const buildDir of buildDirs) {
    for (const entry of readdirSync(buildDir, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue
      const candidate = join(buildDir, entry.name)
      if (existsSync(join(candidate, 'bin')) && existsSync(join(candidate, 'Resources'))) {
        return candidate
      }
    }
  }

  throw new Error(
    `No Electrobun Linux app bundle found under ${relative(projectRoot, buildRoot)}. Run pnpm run build:stable on Linux first.`
  )
}

function createPackageLayout(root, options) {
  rmSync(root, { recursive: true, force: true })
  mkdirSync(root, { recursive: true })

  const appRoot = join(root, 'opt', packageName, 'app')
  cpSync(bundleDir, appRoot, { recursive: true, dereference: true })
  const launcherPath = join(appRoot, 'bin', 'launcher')
  if (!existsSync(launcherPath)) {
    throw new Error(`Electrobun launcher not found at ${relative(projectRoot, launcherPath)}.`)
  }
  chmodSync(launcherPath, 0o755)

  writeExecutable(
    join(root, 'usr', 'bin', binaryName),
    `#!/bin/sh
exec /opt/${packageName}/app/bin/launcher "$@"
`
  )

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
exec "$HERE/opt/${packageName}/app/bin/launcher" "$@"
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
Depends: libgtk-3-0, libwebkit2gtk-4.1-0 | libwebkit2gtk-4.0-37, libayatana-appindicator3-1 | libappindicator3-1
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
    createTarGz(controlRoot, controlTar)
    createTarGz(layoutRoot, dataTar)
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
  cpSync(layoutRoot, payloadPath, { recursive: true, dereference: true })
  writeText(
    specPath,
    `Name: ${packageName}
Version: ${version}
Release: 1
Summary: Local-first CSV anonymization desktop app
License: MIT
URL: ${homepage}
Packager: ${maintainer}
AutoReqProv: no

%description
${description}

%install
rm -rf "%{buildroot}"
mkdir -p "%{buildroot}"
cp -a "%{_sourcedir}/payload/." "%{buildroot}/"

%files
%defattr(-,root,root,-)
/opt/${packageName}
/usr/bin/${binaryName}
/usr/share/applications/${desktopId}
/usr/share/metainfo/${componentId}.metainfo.xml
/usr/share/icons/hicolor
/usr/share/doc/${packageName}/copyright
%license /usr/share/licenses/${packageName}/LICENSE

%changelog
* Thu Jun 18 2026 ${maintainer} - ${version}-1
- Package Electrobun Linux build.
`
  )
  run('rpmbuild', ['--define', `_topdir ${rpmRoot}`, '--target', rpmArch, '-bb', specPath])
  const rpmDir = join(rpmRoot, 'RPMS', rpmArch)
  const rpm = readdirSync(rpmDir).find((name) => name.endsWith('.rpm'))
  if (!rpm) throw new Error(`rpmbuild did not produce an RPM under ${relative(projectRoot, rpmDir)}.`)
  copyFileSync(join(rpmDir, rpm), join(packagesDir, rpm))
}

function buildAppImage(appDir, appimagetool) {
  const outputPath = join(packagesDir, `CSVAnonymizer-${version}-${appImageArch}.AppImage`)
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

function createTarGz(sourceDir, outputPath) {
  mkdirSync(dirname(outputPath), { recursive: true })
  run('tar', ['-czf', outputPath, '-C', sourceDir, '.'], {
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
