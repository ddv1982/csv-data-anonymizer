# Release Workflow

Use this workflow when publishing downloadable CSV Anonymizer desktop artifacts.

The project uses Bun for dependency installation and package scripts. The committed `bun.lock` is the release lockfile.

## Version And Tag

Update `package.json`, `CHANGELOG.md`, and the latest `<release>` entry in `build/linux/io.github.ddv1982.csv-data-anonymizer.metainfo.xml` to the same version and date.

Validate metadata before tagging:

```bash
bun run release:check -- --expected-tag v1.0.0
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
- Linux: Electrobun `.tar.zst`, setup `.tar.gz`, `.deb`, `.rpm`, AppImage, update metadata, APT repository, APT repository setup `.deb`, `install-apt-repo.sh`, keyring, checksum sidecars, and detached `.asc` signatures
Windows artifacts are not published yet. Add an Authenticode signing step before enabling Windows release distribution.

Artifacts are written to `dist/electrobun/artifacts/` and uploaded to the draft GitHub Release. Public APT bootstrap assets are also copied into `dist/electrobun/apt-pages/` for GitHub Pages so unauthenticated Linux installs do not depend on GitHub Release asset access.

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

The release workflow exports the archive keyring, repository setup package, setup checksum sidecar, setup checksum signature, and `install-apt-repo.sh` to GitHub Pages because Linux installs use public Pages URLs for the bootstrap flow.

## Windows Notes

Windows distribution is deferred until Authenticode signing is configured. Electrobun builds the current host platform, so a future signed Windows release job should run on a native Windows GitHub-hosted runner.

## Release Behavior

Pushing a `v*` tag triggers `.github/workflows/release.yml`.

The release workflow:

- validates tag, package version, changelog, and Linux metainfo metadata
- creates or refreshes a draft GitHub Release
- builds and verifies signed/notarized macOS arm64 and x64 artifacts
- builds Linux x64 Electrobun artifacts and runs the Electrobun smoke workflow
- wraps Linux output as `.deb`, `.rpm`, and AppImage
- validates Linux package metadata and builds a signed APT repository
- stages `install-apt-repo.sh` with the pinned APT signing key fingerprint
- publishes public APT bootstrap assets in the APT Pages artifact for installer-side signature verification
- uploads macOS and Linux release assets
- deploys the APT repository to GitHub Pages
- publishes the GitHub Release only after all platform and APT jobs succeed

## Local Validation

Before pushing a release tag, run on the host platform:

```bash
bun install --frozen-lockfile
bun run typecheck
bun run test
bun run build:stable
bun run smoke:electrobun
bun run artifacts:electrobun:check
bun run release:check -- --expected-tag v1.0.0
```

On Linux, also validate package-manager artifacts:

```bash
bun run linux:packages
bun run linux:metadata:check
bun run apt:repo:check
```
