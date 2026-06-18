# Release Workflow

Use this workflow when publishing downloadable CSV Anonymizer desktop artifacts.

The app runtime is native Rust. Node is used only for repository release scripts.

## Version And Tag

Update `package.json`, `Cargo.toml`, `CHANGELOG.md`, and the latest `<release>` entry in `build/linux/io.github.ddv1982.csv-data-anonymizer.metainfo.xml` to the same version and date.

Validate metadata before tagging:

```bash
node scripts/check-release-metadata.mjs --expected-tag v1.0.0
```

Then commit, tag, and push:

```bash
git add -A
git commit -m "lore(release): v1.0.0"
git tag v1.0.0
git push origin main
git push origin v1.0.0
```

## Artifact Model

The release workflow builds Rust artifacts:

- macOS: signed and notarized `.dmg`, plus a signed/notarized `.app.tar.gz`
- Linux: portable `.tar.gz`, `.deb`, `.rpm`, AppImage, APT repository, APT repository setup `.deb`, `install-apt-repo.sh`, keyring, checksum sidecars, and detached `.asc` signatures

Artifacts are written to `dist/rust/artifacts/` and uploaded to the draft GitHub Release. Public APT bootstrap assets are copied into `dist/rust/apt-pages/` for GitHub Pages so unauthenticated Linux installs do not depend on GitHub Release asset access.

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

Linux release jobs run on Ubuntu and build the current host platform from the Rust binary.

Configure these signing inputs:

- `DEB_SIGNING_PRIVATE_KEY`: base64-encoded ASCII-armored GPG private key
- `DEB_SIGNING_KEY_FINGERPRINT`: full fingerprint for the Linux signing key
- `DEB_SIGNING_KEY_PASSPHRASE`: passphrase for the Linux signing key
- `DEB_SIGNING_PUBLIC_KEY`: repository variable containing the base64-encoded ASCII-armored public key

The names are historical. The key signs Linux archives, `.deb`, `.rpm`, AppImage, APT metadata, the APT setup package checksum, and release sidecar signatures.

The APT repository is generated under `dist/rust/apt-pages/apt` and deployed with GitHub Pages. Repository Pages must be configured to use GitHub Actions as the Pages source.

The release workflow exports the archive keyring, repository setup package, setup checksum sidecar, setup checksum signature, and `install-apt-repo.sh` to GitHub Pages because Linux installs use public Pages URLs for the bootstrap flow.

## Release Behavior

Pushing a `v*` tag triggers `.github/workflows/release.yml`.

The release workflow:

- validates tag, package version, changelog, Rust workspace, and Linux metainfo metadata
- creates or refreshes a draft GitHub Release
- builds and verifies signed/notarized macOS arm64 and x64 artifacts
- builds the Linux x64 Rust binary and runs the Rust smoke workflow
- wraps Linux output as `.deb`, `.rpm`, AppImage, and a portable `.tar.gz`
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
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release -p csv-anonymizer-app
node scripts/rust-smoke.mjs
node scripts/check-release-metadata.mjs --expected-tag v1.0.0
```

On macOS, validate app packaging:

```bash
node scripts/package-rust-macos.mjs --skip-missing-tools
node scripts/check-rust-artifacts.mjs --platform macos
```

On Linux, also validate package-manager artifacts:

```bash
node scripts/package-rust-linux.mjs
python3 scripts/validate_linux_package_metadata.py "dist/rust/artifacts/*.deb" "dist/rust/artifacts/*.rpm"
node scripts/check-apt-repository.mjs
node scripts/check-apt-installer.mjs
```
