import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import test from 'node:test'

const root = process.cwd()
const action = fs.readFileSync(path.join(root, '.github/actions/validate-build/action.yml'), 'utf8')
const ci = fs.readFileSync(path.join(root, '.github/workflows/ci.yml'), 'utf8')
const release = fs.readFileSync(path.join(root, '.github/workflows/release.yml'), 'utf8')

test('shared validation runs static gates before dependency installation', () => {
  const metadata = action.indexOf('- name: Validate release metadata')
  const docs = action.indexOf('- name: Validate documented commands')
  const tooling = action.indexOf('- name: Test release tooling')
  const install = action.indexOf('- name: Install frontend dependencies')
  assert.ok(metadata >= 0 && metadata < docs && docs < tooling && tooling < install)
  assert.match(action, /cargo install cargo-audit --locked --version "=0\.22\.2"/)
  assert.match(action, /cargo install cargo-machete --locked --version "=0\.9\.2"/)
})

test('CI and release use the canonical Linux metadata package command', () => {
  for (const workflow of [ci, release]) {
    assert.match(workflow, /npm run linux:metadata:check -- \\\n\s+--json-report linux-package-metadata-report\.json/)
    assert.doesNotMatch(workflow, /python3 scripts\/validate_linux_package_metadata\.py/)
  }
})

test('release validation and publication dependencies remain fail-closed', () => {
  assert.match(release, /uses: \.\/\.github\/actions\/validate-build[\s\S]*expected-tag: \$\{\{ github\.ref_name \}\}/)
  assert.match(release, /publish-apt-repository:[\s\S]*needs: \[build-macos-release, build-linux-release\]/)
  assert.match(release, /publish-release:[\s\S]*needs: \[create-release, build-macos-release, build-linux-release, publish-apt-repository\]/)
  assert.ok(release.indexOf('- name: Validate Linux package metadata') < release.indexOf('- name: Build signed APT repository and checksums'))
})
