import { readFile, writeFile } from 'node:fs/promises';
import { basename } from 'node:path';

const args = process.argv.slice(2);

function readOption(name) {
  const index = args.indexOf(name);
  if (index === -1) {
    return undefined;
  }

  const value = args[index + 1];
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }

  return value;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

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

const expectedTag = readOption('--expected-tag');
const releaseNotesPath = readOption('--write-notes');
const packageJson = JSON.parse(await readFile('package.json', 'utf-8'));

if (!validateSemver(packageJson.version)) {
  throw new Error(`package.json version must be semver-compatible, got ${packageJson.version}`);
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

if (releaseNotesPath) {
  await writeFile(releaseNotesPath, `${release.notes}\n`, 'utf-8');
}

console.log(`Release metadata ok for ${tag} (${basename(process.cwd())})`);
