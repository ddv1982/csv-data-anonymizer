import { readFile, writeFile } from 'node:fs/promises';
import { basename } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { escapeRegExp, readOption } from './command-utils.mjs';

const args = process.argv.slice(2);
const execFileAsync = promisify(execFile);

function findChangelogRelease(changelog, tag) {
  const headingPattern = new RegExp(`^## ${escapeRegExp(tag)} - \\d{4}-\\d{2}-\\d{2}$`);
  const lines = changelog.split(/\r?\n/);
  const headingIndex = lines.findIndex(line => headingPattern.test(line));

  if (headingIndex === -1) {
    return null;
  }

  const date = lines[headingIndex].replace(new RegExp(`^## ${escapeRegExp(tag)} - `), '');
  const sectionLines = [];
  for (const line of lines.slice(headingIndex + 1)) {
    if (line.startsWith('## ')) {
      break;
    }
    sectionLines.push(line);
  }

  return {
    date,
    notes: sectionLines.join('\n').trim()
  };
}

function latestMetainfoRelease(metainfo) {
  const match = metainfo.match(/<release\s+([^>]*)/);
  if (!match) {
    return null;
  }

  const attrs = new Map();
  for (const attr of match[1].matchAll(/([A-Za-z_:][-A-Za-z0-9_:.]*)="([^"]*)"/g)) {
    attrs.set(attr[1], attr[2]);
  }

  const version = attrs.get('version');
  const date = attrs.get('date');
  if (!version || !date) {
    return null;
  }

  return {
    version,
    date
  };
}

function validateSemver(version) {
  return /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(version);
}

function readWorkspaceCargoVersion(cargoToml) {
  const workspacePackageMatch = cargoToml.match(/\[workspace\.package\]([\s\S]*?)(?:\n\[|$)/);
  if (!workspacePackageMatch) {
    return null;
  }

  const versionMatch = workspacePackageMatch[1].match(/^\s*version\s*=\s*"([^"]+)"\s*$/m);
  return versionMatch?.[1] ?? null;
}

function readCargoLockPackageVersions(cargoLock, packageNames) {
  const wanted = new Set(packageNames);
  const versions = new Map();
  for (const block of cargoLock.split('[[package]]').slice(1)) {
    const name = block.match(/^name\s*=\s*"([^"]+)"\s*$/m)?.[1];
    if (!name || !wanted.has(name)) {
      continue;
    }
    const version = block.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
    versions.set(name, version ?? null);
  }
  return versions;
}

async function readTrackedFiles() {
  const { stdout } = await execFileAsync('git', ['ls-files'], { maxBuffer: 1024 * 1024 * 10 });
  return stdout.split(/\r?\n/).filter(Boolean);
}

const expectedTag = readOption(args, '--expected-tag');
const releaseNotesPath = readOption(args, '--write-notes');
const packageJson = JSON.parse(await readFile('package.json', 'utf-8'));
const frontendPackageJson = JSON.parse(await readFile('frontend/package.json', 'utf-8'));
const frontendPackageLock = JSON.parse(await readFile('frontend/package-lock.json', 'utf-8'));
const tauriConfig = JSON.parse(await readFile('src-tauri/tauri.conf.json', 'utf-8'));
const linuxTauriConfig = JSON.parse(await readFile('src-tauri/tauri.linux.conf.json', 'utf-8'));
const cargoToml = await readFile('Cargo.toml', 'utf-8');
const cargoLock = await readFile('Cargo.lock', 'utf-8');
const cargoVersion = readWorkspaceCargoVersion(cargoToml);
const cargoLockWorkspacePackages = [
  'csv-anonymizer-app',
  'csv-anonymizer-core',
  'csv-anonymizer-tauri'
];
const cargoLockVersions = readCargoLockPackageVersions(cargoLock, cargoLockWorkspacePackages);

if (!validateSemver(packageJson.version)) {
  throw new Error(`package.json version must be semver-compatible, got ${packageJson.version}`);
}

if (!cargoVersion) {
  throw new Error('Cargo.toml must declare [workspace.package] version');
}

if (cargoVersion !== packageJson.version) {
  throw new Error(`Cargo.toml workspace package version ${cargoVersion} does not match package.json version ${packageJson.version}`);
}

for (const packageName of cargoLockWorkspacePackages) {
  if (!cargoLockVersions.has(packageName)) {
    throw new Error(`Cargo.lock must contain workspace package ${packageName}`);
  }
  const lockedVersion = cargoLockVersions.get(packageName);
  if (lockedVersion !== packageJson.version) {
    throw new Error(`Cargo.lock package ${packageName} version ${lockedVersion} does not match package.json version ${packageJson.version}`);
  }
}

if (frontendPackageJson.version !== packageJson.version) {
  throw new Error(`frontend/package.json version ${frontendPackageJson.version} does not match package.json version ${packageJson.version}`);
}

if (frontendPackageLock.version !== packageJson.version) {
  throw new Error(`frontend/package-lock.json version ${frontendPackageLock.version} does not match package.json version ${packageJson.version}`);
}

if (frontendPackageLock.packages?.['']?.version !== packageJson.version) {
  throw new Error(`frontend/package-lock.json root package version ${frontendPackageLock.packages?.['']?.version} does not match package.json version ${packageJson.version}`);
}

if (tauriConfig.version !== packageJson.version) {
  throw new Error(`src-tauri/tauri.conf.json version ${tauriConfig.version} does not match package.json version ${packageJson.version}`);
}

if (tauriConfig.identifier !== 'io.github.ddv1982.csv-data-anonymizer') {
  throw new Error(`src-tauri/tauri.conf.json identifier must stay io.github.ddv1982.csv-data-anonymizer, got ${tauriConfig.identifier}`);
}

const requiredLinuxIcons = [16, 32, 48, 64, 128, 256, 512, 1024].map(size => `../build/icons/${size}x${size}.png`);
for (const icon of requiredLinuxIcons) {
  if (!tauriConfig.bundle?.icon?.includes(icon)) {
    throw new Error(`src-tauri/tauri.conf.json bundle.icon must include ${icon}`);
  }
  await readFile(icon.replace(/^\.\.\//, '')).catch(() => {
    throw new Error(`required Linux icon file is missing or unreadable: ${icon}`);
  });
}

const forbiddenTrackedFiles = (await readTrackedFiles()).filter(file =>
  file.endsWith('.gguf') ||
  file.includes('/model-cache/') ||
  file.includes('/ollama-cache/') ||
  basename(file).startsWith('llama-server')
);
if (forbiddenTrackedFiles.length > 0) {
  throw new Error(`release must not track Local AI model/runtime artifacts: ${forbiddenTrackedFiles.join(', ')}`);
}

if (packageJson.desktopName !== 'csv-anonymizer.desktop') {
  throw new Error(`package.json desktopName must stay csv-anonymizer.desktop, got ${packageJson.desktopName}`);
}

if (linuxTauriConfig.productName !== packageJson.name) {
  throw new Error(`src-tauri/tauri.linux.conf.json productName ${linuxTauriConfig.productName} must match package.json name ${packageJson.name}`);
}

const linuxDesktopTemplate = '../build/linux/csv-anonymizer.desktop.hbs';
if (linuxTauriConfig.bundle?.linux?.deb?.desktopTemplate !== linuxDesktopTemplate) {
  throw new Error(`src-tauri/tauri.linux.conf.json deb desktopTemplate must be ${linuxDesktopTemplate}`);
}

if (linuxTauriConfig.bundle?.linux?.rpm?.desktopTemplate !== linuxDesktopTemplate) {
  throw new Error(`src-tauri/tauri.linux.conf.json rpm desktopTemplate must be ${linuxDesktopTemplate}`);
}

const tag = expectedTag ?? `v${packageJson.version}`;

if (!tag.startsWith('v')) {
  throw new Error(`release tag must start with "v", got ${tag}`);
}

if (tag !== `v${packageJson.version}`) {
  throw new Error(`release tag ${tag} does not match package.json version ${packageJson.version}`);
}

const changelog = await readFile('CHANGELOG.md', 'utf-8');
const release = findChangelogRelease(changelog, tag);

if (!release?.notes) {
  throw new Error(`CHANGELOG.md must contain a non-empty section headed "## ${tag} - YYYY-MM-DD"`);
}

const metainfoPath = 'build/linux/io.github.ddv1982.csv-data-anonymizer.metainfo.xml';
const metainfo = await readFile(metainfoPath, 'utf-8');
const metainfoRelease = latestMetainfoRelease(metainfo);

if (!metainfoRelease) {
  throw new Error(`${metainfoPath} must contain a <release version="..." date="..."> entry`);
}

if (metainfoRelease.version !== packageJson.version) {
  throw new Error(`${metainfoPath} latest release version ${metainfoRelease.version} does not match package.json version ${packageJson.version}`);
}

if (metainfoRelease.date !== release.date) {
  throw new Error(`${metainfoPath} release date ${metainfoRelease.date} does not match CHANGELOG.md date ${release.date}`);
}

if (!metainfo.includes('<project_license>MIT</project_license>')) {
  throw new Error(`${metainfoPath} must declare <project_license>MIT</project_license>`);
}

if (!metainfo.includes(`<launchable type="desktop-id">${packageJson.desktopName}</launchable>`)) {
  throw new Error(`${metainfoPath} must launch ${packageJson.desktopName}`);
}

const desktopTemplate = await readFile('build/linux/csv-anonymizer.desktop.hbs', 'utf-8');
if (!desktopTemplate.includes('Name=CSV Anonymizer')) {
  throw new Error('build/linux/csv-anonymizer.desktop.hbs must preserve the visible Name=CSV Anonymizer label');
}

if (!desktopTemplate.includes('Exec={{exec}}') || !desktopTemplate.includes('Icon={{icon}}')) {
  throw new Error('build/linux/csv-anonymizer.desktop.hbs must keep Tauri Exec and Icon template variables');
}

if (releaseNotesPath) {
  await writeFile(releaseNotesPath, `${release.notes}\n`, 'utf-8');
}

console.log(`Release metadata ok for ${tag} (${basename(process.cwd())})`);
