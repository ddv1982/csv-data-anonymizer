# Release Workflow

Use this workflow when publishing downloadable CSV Anonymizer desktop artifacts.

## Version And Tag

Update `package.json`, `CHANGELOG.md`, and the latest `<release>` entry in `build/linux/io.github.ddv1982.csv-data-anonymizer.metainfo.xml` to the same version and date.

Validate metadata before tagging:

```bash
pnpm run release:check -- --expected-tag v1.0.0
```

Then commit, tag, and push:

```bash
git add -A
git commit -m "chore(release): v1.0.0"
git tag v1.0.0
git push origin main
git push origin v1.0.0
```

## Artifact Model

The release workflow builds Electrobun artifacts only:

- macOS: signed/notarized `.dmg`, `.app.tar.zst`, update metadata, and patch artifacts when enabled
- Linux: Electrobun `.tar.zst`, setup `.tar.gz`, `.deb`, `.rpm`, AppImage, update metadata, APT repository, and detached `.asc` signatures
- Windows: Electrobun setup `.zip`, `.tar.zst`, and update metadata

Artifacts are written to `dist/electrobun/artifacts/` and uploaded to the draft GitHub Release.

## macOS Prerequisites

Configure these GitHub Actions secrets before pushing a release tag:

- `CSC_LINK`: base64-encoded Developer ID Application `.p12` certificate
- `CSC_KEY_PASSWORD`: password for the `.p12` certificate
- `ELECTROBUN_DEVELOPER_ID`: Developer ID identity passed to `codesign`
- `ELECTROBUN_TEAMID`: Apple Developer Team ID
- `APPLE_API_KEY`: base64-encoded App Store Connect `.p8` private key content
- `APPLE_API_KEY_ID`: App Store Connect key ID
- `APPLE_API_ISSUER`: App Store Connect issuer ID

The workflow imports the certificate into a temporary keychain, sets `ELECTROBUN_APPLEAPIKEYPATH`, builds stable Electrobun artifacts, then verifies the `.app` and `.dmg` with `codesign`, `stapler`, and `spctl`.

## Linux Prerequisites

Linux release jobs run on Ubuntu and build the current host platform with CEF enabled.

Configure these signing inputs:

- `DEB_SIGNING_PRIVATE_KEY`: base64-encoded ASCII-armored GPG private key
- `DEB_SIGNING_KEY_FINGERPRINT`: full fingerprint for the Linux signing key
- `DEB_SIGNING_KEY_PASSPHRASE`: passphrase for the Linux signing key
- `DEB_SIGNING_PUBLIC_KEY`: repository variable containing the base64-encoded ASCII-armored public key

The names are historical. The key now signs Linux Electrobun archives, `.deb`, `.rpm`, AppImage, APT metadata, the APT setup package checksum, and release sidecar signatures.

The APT repository is generated under `dist/electrobun/apt-pages/apt` and deployed with GitHub Pages. Repository Pages must be configured to use GitHub Actions as the Pages source.

## Windows Notes

Windows release jobs run on the native `windows-2025` GitHub-hosted runner because Electrobun builds the current host platform. The workflow publishes unsigned Windows artifacts unless a future Authenticode signing step is added.

## Release Behavior

Pushing a `v*` tag triggers `.github/workflows/release.yml`.

The release workflow:

- validates tag, package version, changelog, and Linux metainfo metadata
- creates or refreshes a draft GitHub Release
- builds and verifies signed/notarized macOS arm64 and x64 artifacts
- builds Linux x64 Electrobun artifacts and runs the Electrobun smoke workflow
- wraps Linux output as `.deb`, `.rpm`, and AppImage
- validates Linux package metadata and builds a signed APT repository
- builds Windows x64 Electrobun artifacts and runs the Electrobun smoke workflow
- uploads macOS, Linux, and Windows release assets
- deploys the APT repository to GitHub Pages
- publishes the GitHub Release only after all platform and APT jobs succeed

## Local Validation

Before pushing a release tag, run on the host platform:

```bash
pnpm install --frozen-lockfile
pnpm run typecheck
pnpm test
pnpm run build:stable
pnpm run smoke:electrobun
pnpm run artifacts:electrobun:check
pnpm run release:check -- --expected-tag v1.0.0
```

On Linux, also validate package-manager artifacts:

```bash
pnpm run linux:packages
pnpm run linux:metadata:check
pnpm run apt:repo:check
```
