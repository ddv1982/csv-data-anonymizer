# Release Workflow

Use this workflow when publishing downloadable CSV Anonymizer desktop artifacts.

The app runtime is Tauri with a bundled Vite frontend. Node is used for the
frontend build and repository release scripts.

## Version And Tag

Update `package.json`, `frontend/package.json`, `frontend/package-lock.json`,
`Cargo.toml`, `src-tauri/tauri.conf.json`, `CHANGELOG.md`, and the latest
`<release>` entry in `build/linux/io.github.ddv1982.csv-data-anonymizer.metainfo.xml`
to the same version and date.

Validate metadata before tagging:

```bash
node scripts/check-release-metadata.mjs --expected-tag v1.0.0
```

The metadata check also gates the Linux Tauri package identity: `src-tauri/tauri.linux.conf.json` keeps the installed desktop file at `csv-anonymizer.desktop`, while `build/linux/csv-anonymizer.desktop.hbs` keeps the visible app name as CSV Anonymizer.

Then commit, tag, and push:

```bash
git add -A
git commit -m "lore(release): v1.0.0"
git tag v1.0.0
git push origin main
git push origin v1.0.0
```

## Artifact Model

The release workflow builds Tauri desktop artifacts:

- macOS: signed and notarized `.dmg` installers named with standard architecture suffixes, for example `CSV.Anonymizer_1.0.0_aarch64.dmg` and `CSV.Anonymizer_1.0.0_x64.dmg`
- Linux: `.deb`, `.rpm`, AppImage, APT repository, APT repository setup `.deb`, setup checksum sidecar, setup checksum signature, and `install-apt-repo.sh`

Artifacts are written to `dist/rust/artifacts/`. The GitHub Release intentionally publishes only user-facing installers and APT bootstrap files; the archive keyring stays on GitHub Pages because it is consumed by `install-apt-repo.sh`.

CI and release jobs install the Tauri CLI with the pinned workflow `TAURI_CLI_VERSION`, which should match the Tauri crate version resolved in `Cargo.lock`.

## macOS Prerequisites

Configure these GitHub Actions secrets before pushing a release tag:

- `CSC_LINK`: base64-encoded Developer ID Application `.p12` certificate
- `CSC_KEY_PASSWORD`: password for the `.p12` certificate
- `MACOS_DEVELOPER_ID`: optional Developer ID identity passed to `codesign`
- `MACOS_TEAM_ID`: optional Apple Developer Team ID
- `APPLE_API_KEY`: base64-encoded App Store Connect `.p8` private key content
- `APPLE_API_KEY_ID`: App Store Connect key ID
- `APPLE_API_ISSUER`: App Store Connect issuer ID

The workflow imports the certificate into a temporary keychain, signs the `.app`, notarizes and staples it, creates a `.dmg`, signs/notarizes/staples the `.dmg`, then verifies both artifacts.

The previous `ELECTROBUN_DEVELOPER_ID` and `ELECTROBUN_TEAMID` secrets are still accepted as fallback names while repository secrets are being renamed.

## Linux Prerequisites

Linux release jobs run on Ubuntu and build the current host platform through
Tauri, embedding `frontend/dist` into the desktop bundle.

Configure these signing inputs:

- `DEB_SIGNING_PRIVATE_KEY`: base64-encoded ASCII-armored GPG private key
- `DEB_SIGNING_KEY_FINGERPRINT`: full fingerprint for the Linux signing key
- `DEB_SIGNING_KEY_PASSPHRASE`: passphrase for the Linux signing key
- `DEB_SIGNING_PUBLIC_KEY`: repository variable containing the base64-encoded ASCII-armored public key

The names are historical. The key signs APT metadata and the APT setup package checksum.

The APT repository is generated under `dist/rust/apt-pages/apt` and deployed with GitHub Pages. Repository Pages must be configured to use GitHub Actions as the Pages source.

The release workflow exports the archive keyring, repository setup package, setup checksum sidecar, setup checksum signature, and `install-apt-repo.sh` to GitHub Pages because Linux installs use public Pages URLs for the bootstrap flow. The installer script itself is not published with a detached signature; it authenticates the setup package through the pinned key and signed checksum.

## Release Behavior

Pushing a `v*` tag triggers `.github/workflows/release.yml`.

The release workflow:

- validates tag, package version, changelog, Rust workspace, and Linux metainfo metadata
- audits frontend dependencies with `npm run frontend:audit`
- creates or refreshes a draft GitHub Release
- builds and verifies signed/notarized macOS arm64 and x64 artifacts
- builds the frontend once for Tauri packaging
- validates the prebuilt frontend contains `index.html` and non-empty CSS assets before Tauri consumes it
- builds Linux output as `.deb`, `.rpm`, and AppImage through Tauri
- validates Linux package metadata and builds a signed APT repository
- stages `install-apt-repo.sh` with the pinned APT signing key fingerprint and validates the rendered installer keeps that effective fingerprint
- publishes public APT bootstrap assets in the APT Pages artifact for installer-side signature verification
- uploads macOS and Linux release assets
- deploys the APT repository to GitHub Pages
- publishes the GitHub Release only after all platform and APT jobs succeed

## Local Validation

Before pushing a release tag, run on the host platform:

```bash
cargo fmt --all --check
npm ci --prefix frontend
npm run frontend:audit
npm run frontend:build
CSV_ANONYMIZER_USE_PREBUILT_FRONTEND=1 scripts/build_frontend_for_tauri.sh
npm run test
npm run lint
node scripts/rust-smoke.mjs
node scripts/check-release-metadata.mjs --expected-tag v1.0.0
```

On macOS, validate app packaging:

```bash
cd src-tauri
CSV_ANONYMIZER_USE_PREBUILT_FRONTEND=1 cargo tauri build --bundles app
```

On Linux, also validate package-manager artifacts:

```bash
node scripts/package-tauri-linux.mjs
python3 scripts/validate_linux_package_metadata.py "dist/rust/artifacts/*.deb" "dist/rust/artifacts/*.rpm"
node scripts/check-apt-repository.mjs
node scripts/check-apt-installer.mjs
```
