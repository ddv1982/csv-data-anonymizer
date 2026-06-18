#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, rmSync, copyFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const args = new Set(process.argv.slice(2));
const skipBuild = args.has('--skip-build');
const target = process.env.TAURI_TARGET ?? 'x86_64-unknown-linux-gnu';
const bundleRootCandidates = [
  path.join(repoRoot, 'target', target, 'release', 'bundle'),
  path.join(repoRoot, 'target', 'release', 'bundle'),
  path.join(repoRoot, 'src-tauri', 'target', target, 'release', 'bundle'),
  path.join(repoRoot, 'src-tauri', 'target', 'release', 'bundle'),
];
const artifactDir = path.join(repoRoot, 'dist', 'rust', 'artifacts');

function run(command, commandArgs, options = {}) {
  const result = spawnSync(command, commandArgs, {
    cwd: repoRoot,
    stdio: 'inherit',
    shell: false,
    ...options,
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function bundleDirectory() {
  for (const candidate of bundleRootCandidates) {
    if (existsSync(candidate)) return candidate;
  }
  return bundleRootCandidates[0];
}

function collectArtifacts(root) {
  const candidates = [
    ['deb', '.deb'],
    ['rpm', '.rpm'],
    ['appimage', '.AppImage'],
  ];
  const artifacts = [];

  for (const [directoryName, extension] of candidates) {
    const directory = path.join(root, directoryName);
    if (!existsSync(directory)) continue;
    for (const entry of readdirSync(directory)) {
      if (entry.endsWith(extension)) {
        artifacts.push(path.join(directory, entry));
      }
    }
  }

  return artifacts;
}

if (!skipBuild) {
  if (process.env.CSV_ANONYMIZER_USE_PREBUILT_FRONTEND === '1') {
    run('bash', ['scripts/build_frontend_for_tauri.sh'], {
      env: { ...process.env, CSV_ANONYMIZER_USE_PREBUILT_FRONTEND: '1' },
    });
  }

  run('cargo', ['tauri', 'build', '--target', target], {
    cwd: path.join(repoRoot, 'src-tauri'),
  });
} else {
  run('bash', ['scripts/build_frontend_for_tauri.sh'], {
    env: { ...process.env, CSV_ANONYMIZER_USE_PREBUILT_FRONTEND: '1' },
  });
}

const root = bundleDirectory();
const artifacts = collectArtifacts(root);
if (artifacts.length === 0) {
  console.error(`No Tauri Linux artifacts found under ${root}.`);
  process.exit(1);
}

rmSync(artifactDir, { recursive: true, force: true });
mkdirSync(artifactDir, { recursive: true });
for (const artifact of artifacts) {
  copyFileSync(artifact, path.join(artifactDir, path.basename(artifact)));
}

console.log(`Copied ${artifacts.length} Tauri Linux artifact(s) to ${artifactDir}.`);
