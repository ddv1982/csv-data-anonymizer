# Dependency Audit Follow-Ups

Last reviewed: 2026-07-02

Cleanup coordination note: the behavior-preserving cleanup pass tracked in
`docs/cleanup-phased-plan-2026-07-01.md` intentionally avoided dependency
version churn. Keep future dependency upgrades separate from cleanup refactors
unless the cleanup is specifically about release/package scripts.

This project treats `cargo audit` warnings as review inputs, not automatic
release blockers. As of this review, app-owned XML and phone-number paths use
`quick-xml 0.41.0`; the only remaining `quick-xml` vulnerability path is the
Tauri `plist` dependency, which is temporarily ignored by `scripts/cargo-audit.mjs`
only while `cargo tree -i quick-xml@0.39.4 --depth 1` resolves directly to
`plist`.

The scheduled `.github/workflows/dead-code.yml` maintenance workflow now also
runs `npm run cargo:audit:required`, so new RustSec advisories are checked even
when no source change or release workflow is running.

## Current Audit State

- RustSec database: 2026-07-02 10:52:02 +02:00, 1149 advisories.
- Lockfile dependency count: 531.
- Vulnerabilities: 2 temporarily ignored Tauri/plist-transitive `quick-xml`
  advisories: `RUSTSEC-2026-0194` and `RUSTSEC-2026-0195`.
- Informational warnings: 17 unmaintained packages and 1 unsound package.

## Latest Upgrade Research Snapshot

As of 2026-07-02, the stable Tauri line is still Tauri 2.x, not Tauri 3.x.
The local manifests and lockfile now use `tauri 2.11.5` and
`tauri-build 2.6.3`. The matching Cargo CLI release is `tauri-cli 2.11.5`;
the npm CLI wrapper is also `2.11.5`. `@tauri-apps/api` is current at
`2.11.1`. `tauri-runtime` remains on `2.11.3` because Cargo reports that as the
latest published version for that crate.

The Linux migration target found in upstream research is GTK4 plus WebKitGTK
6.0. It is not "GTK6". The main upstream migration PR is still open and marked
for Tauri 3.0:
<https://github.com/tauri-apps/tauri/pull/14684>.

Current package and dependency observations:

- Tauri patch set: `tauri 2.11.5`, `tauri-build 2.6.3`,
  `tauri-runtime-wry 2.11.4`, and `tauri-cli 2.11.5` have been applied.
  `tauri-plugin-dialog 2.7.1` is current.
- Frontend patch set: `@vitejs/plugin-react 6.0.3`, `eslint 10.6.0`,
  `knip 6.23.0`, `typescript-eslint 8.62.1`, and `vite 8.1.2` have been
  applied.
- Frontend major/typing cleanup: `lucide-react 1.22.0` and
  `@types/node 26.0.1` have been applied. CI workflows now run Node 26.
  Package engines still allow `>=22.13.0`, so avoid relying on Node 26-only
  runtime APIs unless the package engine floor is raised too.
- Rust patch/minor set: `open 5.3.6`, `unicode-segmentation 1.13.3`, and
  `criterion 0.8.2` have been applied.
- Rust major/API set: `quick-xml 0.41.0` has been applied for app-owned XML and
  phone-number paths; `reqwest 0.13.4` and `rand 0.10.1` remain available and
  should be handled separately from patch updates.
- `phonenumber 0.3.10+9.0.33` now routes app-owned phone metadata through
  `quick-xml 0.41.0`, but its all-target `postcard -> heapless ->
  atomic-polyfill` warning remains.
- `urlpattern 0.6.0` avoids the `unic-*` path, but Tauri still pulls
  `urlpattern 0.3.0` through `tauri-utils`, so this should wait for a Tauri
  update rather than a local semver-major override.

## Stable-Track Upgrade Plan

This is the active plan for the stable dependency line. It intentionally does
not prepare for Tauri 3, GTK4/WebKitGTK 6.0, or the Linux `xdg-portal` dialog
backend yet. Those items remain monitoring-only until there is a decision to
start a Tauri 3 migration branch.

Status: Phase 0 baseline capture, Phase 1 current-major upgrades, frontend
outdated cleanup, and Node 26 CI alignment were completed on 2026-07-01. Phase
2 remains a set of focused future PRs.

### Phase 0: Baseline Before Changing Versions

Goal: make the next dependency PR easy to review and easy to roll back.

Status: completed on 2026-07-01.

Actions:

- Capture `npm outdated --prefix frontend --json`,
  `cargo update --workspace --dry-run --verbose`, and
  `npm run cargo:audit:required` output in the PR notes.
- Keep documentation/CI maintenance changes separate from dependency version
  changes where possible.
- Re-run `cargo tree --target x86_64-unknown-linux-gnu -i gtk --workspace`,
  `cargo tree --target x86_64-unknown-linux-gnu -i glib --workspace`, and
  `cargo tree --target x86_64-unknown-linux-gnu -i urlpattern --workspace`
  after each Tauri movement.

Exit criteria:

- No source changes yet.
- Current audit warnings are understood and attributable to the existing paths
  in this document.

### Phase 1: Current-Major Patch Upgrades

Goal: take low-risk updates that keep the same major lines.

Status: completed on 2026-07-01.

Actions:

- Update Tauri 2 patch versions together:
  - workspace `tauri` to `2.11.5`
  - workspace `tauri-build` to `2.6.3`
  - `.github/workflows/ci.yml` `TAURI_CLI_VERSION` to `2.11.5`
  - `.github/workflows/release.yml` `TAURI_CLI_VERSION` to `2.11.5`
- Refresh `Cargo.lock` with package-specific updates first, then inspect the
  diff:
  `cargo update -p tauri -p tauri-build -p tauri-runtime -p tauri-runtime-wry`.
- Update frontend patch dependencies:
  `@vitejs/plugin-react`, `eslint`, `knip`, `typescript-eslint`, and `vite`.
- `@types/node` was later moved to `26.0.1` during the frontend outdated
  cleanup. CI `setup-node` pins were also moved to Node 26, but package engines
  still allow `>=22.13.0`; raise the package engine floor before introducing
  Node 26-only runtime APIs.
- Update Rust patch/minor dependencies that do not imply API rewrites:
  `open`, `unicode-segmentation`, and `criterion`.

Verification:

- `npm ci --prefix frontend`
- `npm run frontend:audit`
- `npm run cargo:audit:required`
- `npm run test`
- `npm run lint`
- `npm run release:check`
- `npm run frontend:e2e`
- `npm run frontend:a11y`
- `node scripts/rust-smoke.mjs`
- On Linux CI or a Linux builder: `npm run linux:package-manager:check`

Expected audit result:

- The GTK3/GLib and `unic-*` warnings probably remain on Tauri 2.x.
- The `atomic-polyfill` warning remains unless upstream `phonenumber` changes
  its `postcard -> heapless` path.

Implementation notes:

- `cargo update -p tauri -p tauri-build -p tauri-runtime -p tauri-runtime-wry
  -p open -p unicode-segmentation -p criterion` updated `tauri`,
  `tauri-runtime-wry`, and `open`, and removed the now-unused `pathdiff`
  package. `tauri-build`, `criterion`, and `unicode-segmentation` were already
  resolved at the target versions. `tauri-runtime` stayed on `2.11.3` because
  that is the latest published crate version.
- After `npm ci --prefix frontend`, `npm outdated --prefix frontend --json`
  returns `{}`.
- Local verification passed: `npm ci --prefix frontend`,
  `npm run frontend:audit`, `npm run cargo:audit:required`, `npm run test`,
  `npm run lint`, `npm run release:check`, `npm run frontend:e2e`,
  `npm run frontend:a11y`, `npm run deadcode:required`, and
  `node scripts/rust-smoke.mjs`.
- GitHub Actions workflow pins were updated from Node 24 to Node 26 for CI,
  release, and scheduled dependency scans.
- `npm run linux:package-manager:check` was not run locally because this pass
  was performed on macOS; keep it as a Linux CI/builder check.

### Phase 2: Non-Tauri Rust Major/API Library Upgrades

Goal: reduce dependency drift without mixing runtime framework migration into
the same change.

Actions:

- Upgrade `quick-xml` to `0.41.x` in a focused PR. Check XML fixture parsing,
  local AI manifest/config handling, and release metadata validation.
- Upgrade `reqwest` to `0.13.x` in a focused PR. Check local AI model download
  behavior, blocking client usage, TLS feature selection, and binary size.
- Upgrade `rand` to `0.10.x` in a focused PR. Check anonymized sample stability
  expectations and any API changes around RNG construction.
Verification:

- Use the full Phase 1 verification set for each major upgrade PR.
- For detector-affecting changes, add or refresh fixtures before changing
  behavior.

## Warnings To Monitor

### Tauri Linux GTK3 / GLib Stack

Warnings:

- `atk`, `atk-sys`, `gdk`, `gdk-sys`, `gdkwayland-sys`, `gdkx11`,
  `gdkx11-sys`, `gtk`, `gtk-sys`, `gtk3-macros`: GTK3 bindings are
  unmaintained.
- `glib`: `RUSTSEC-2024-0429`, unsound `VariantStrIter` implementation,
  patched in `glib >=0.20.0`.
- `proc-macro-error`: transitive through `glib-macros` and `gtk3-macros`.

Current dependency path on Linux:

```text
tauri -> tauri-runtime-wry / tauri-runtime / wry / tao / muda / webkit2gtk -> gtk -> glib
```

Target checks:

- `cargo tree --target x86_64-unknown-linux-gnu -i gtk --workspace` shows the
  Tauri Linux runtime path above.
- `cargo tree --target x86_64-apple-darwin -i gtk --workspace` and
  `cargo tree --target x86_64-pc-windows-msvc -i gtk --workspace` print nothing.

Research status:

- Tauri issue <https://github.com/tauri-apps/tauri/issues/12561> tracks
  migrating `tauri-runtime-wry` from GTK3 to GTK4 to address the GLib advisory.
- Tauri PR <https://github.com/tauri-apps/tauri/pull/14684> tracks the
  GTK4/WebKitGTK 6.0 migration. It is open, marked as a Linux breaking change,
  and assigned to the Tauri 3.0 milestone.
- `tauri-plugin-dialog` PR
  <https://github.com/tauri-apps/plugins-workspace/pull/3176> tracks switching
  the default Linux dialog backend from GTK3 to XDG Desktop Portal for Tauri 3.
- Wry still documents WebKitGTK/GTK as Linux requirements:
  <https://github.com/tauri-apps/wry/blob/dev/README.md#linux>.
- The Tauri Linux prerequisite docs still require `libwebkit2gtk-4.1-dev`:
  <https://tauri.app/start/prerequisites/#linux>.

Decision:

- Do not fork or override GTK/GLib/Tauri crates locally. The real fix is the
  upstream GTK4/WebKitGTK 6 migration.
- Do not describe the target as GTK6. The target is GTK4 plus WebKitGTK 6.0.
- Keep Tauri patch releases current, but expect Tauri 2.x to keep these warnings
  until the GTK4 migration lands.
- Do not start a Tauri 3, GTK4/WebKitGTK 6.0, or `xdg-portal` preparation
  branch yet. Re-evaluate only when the project explicitly chooses to start that
  migration.

### `unic-*` Through `urlpattern`

Warnings:

- `unic-char-property`, `unic-char-range`, `unic-common`, `unic-ucd-ident`,
  `unic-ucd-version`: `rust-unic` crates are unmaintained.

Current dependency path:

```text
tauri-utils -> urlpattern 0.3.0 -> unic-ucd-ident -> unic-*
```

Research status:

- `urlpattern` 0.6.0 no longer depends on `unic-*`; it depends on
  `icu_properties` instead: <https://crates.io/crates/urlpattern>.
- Tauri issue <https://github.com/tauri-apps/tauri/issues/14345> says the
  `unic-ucd-ident` advisory has to wait for Tauri v3 because of MSRV
  constraints.

Decision:

- Do not override `urlpattern` under `tauri-utils`; it is Tauri's ACL parsing
  dependency, and a semver-major `urlpattern` override is not a safe local patch.
- Track Tauri v3 / `tauri-utils` updates for the `urlpattern >=0.6` transition.

### `atomic-polyfill` Through `phonenumber`

Warning:

- `atomic-polyfill`: unmaintained, recommended alternative is
  `portable-atomic`.

Current all-target dependency path:

```text
csv-anonymizer-core -> phonenumber -> postcard -> heapless -> atomic-polyfill
```

Research status:

- `phonenumber 0.3.10+9.0.33` now uses `quick-xml 0.41.0`, clearing the
  vulnerable phone-metadata XML path, but still depends on `postcard ^1.1`:
  <https://crates.io/crates/phonenumber>.
- `postcard` issue <https://github.com/jamesmunns/postcard/issues/223> tracks
  the `atomic-polyfill -> heapless` warning and points toward a future postcard
  release path.
- `heapless` has already merged a switch from `atomic-polyfill` to
  `portable-atomic`: <https://github.com/rust-embedded/heapless/pull/328>.
  The remaining blocker is getting that newer heapless path through postcard and
  then through phonenumber.

Possible alternative:

- `rlibphonenumber` is an actively updated libphonenumber-style crate and avoids
  the `postcard -> heapless` path, but it currently requires Rust 1.88 and has a
  different API: <https://crates.io/crates/rlibphonenumber>.

Decision:

- Keep `phonenumber` for now. It gives mature libphonenumber-style behavior with
  a lower MSRV than `rlibphonenumber`.
- Reconsider replacing it only as a focused detector-quality spike that compares
  fixture behavior, binary/dependency impact, MSRV impact, and API fit.
- Otherwise wait for `phonenumber` to pick up a postcard/heapless path that no
  longer depends on `atomic-polyfill`.

## Improvement Backlog

1. Keep weekly scheduled `cargo audit` active through
   `.github/workflows/dead-code.yml`.
2. Periodically run `cargo update -p tauri --dry-run --verbose` and take patch
   releases when they pass the full test/release gates.
3. Passively watch Tauri 3 / GTK4 migration status:
   <https://github.com/tauri-apps/tauri/pull/14684>.
4. Watch `tauri-utils` moving from `urlpattern 0.3` to `urlpattern >=0.6`.
5. Watch `phonenumber` and `postcard` for a release chain that removes
   `atomic-polyfill`.
6. If the Rust MSRV target rises to at least 1.88, run a spike comparing
   `rlibphonenumber` against the existing phone detection fixtures before
   changing runtime dependencies.

## Complexity Refactors

The multilingual detection work removed the detection-specific
`clippy::too_many_lines` warnings. The later behavior-preserving cleanup pass
also split the unrelated quick generation and preflight readiness hotspots that
were previously listed here. Keep future detector behavior changes separate from
unrelated cleanup so review remains focused.
