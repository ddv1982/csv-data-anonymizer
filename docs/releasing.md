# Release Workflow

Use this workflow when publishing downloadable CSV Anonymizer desktop artifacts.

## Version And Tag

Update `package.json` and add release notes to `CHANGELOG.md` using this heading format:

```md
## v1.0.0 - YYYY-MM-DD
- Release note
```

Also update the latest `<release>` entry in `build/linux/io.github.ddv1982.csv-data-anonymizer.metainfo.xml` to the same version and date.

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

## macOS Prerequisites

macOS release artifacts use Electron Builder with Developer ID signing, hardened runtime, and notarization. Configure these GitHub Actions secrets before pushing a release tag:

- `CSC_LINK`: base64-encoded Developer ID Application `.p12` certificate
- `CSC_KEY_PASSWORD`: password for the `.p12` certificate
- `APPLE_API_KEY`: base64-encoded App Store Connect `.p8` private key content
- `APPLE_API_KEY_ID`: App Store Connect key ID
- `APPLE_API_ISSUER`: App Store Connect issuer ID

The workflow decodes `APPLE_API_KEY` to a temporary `AuthKey_<APPLE_API_KEY_ID>.p8` file for Electron Builder. It passes `-c.forceCodeSigning=true` only in release CI, so local unsigned packaging remains possible.

The Electron Builder macOS config uses:

- `hardenedRuntime: true`
- `notarize: true`
- `build/entitlements.mac.plist`
- `build/entitlements.mac.inherit.plist`

For Electron 42, the entitlements intentionally include JIT support and do not include `com.apple.security.cs.allow-unsigned-executable-memory`.

## Linux Prerequisites

Linux release artifacts use Electron Builder on Ubuntu to produce:

- `AppImage`
- `deb`
- `rpm`

Linux packages are signed as detached GPG `.asc` signatures on `main` pushes before CI uploads the reusable Linux artifact bundle. Configure these GitHub Actions inputs before pushing a release tag:

- `DEB_SIGNING_PRIVATE_KEY`: base64-encoded ASCII-armored GPG private key
- `DEB_SIGNING_KEY_FINGERPRINT`: full fingerprint for the Linux artifact signing key
- `DEB_SIGNING_KEY_PASSPHRASE`: passphrase for the Linux artifact signing key
- `DEB_SIGNING_PUBLIC_KEY`: repository variable containing the base64-encoded ASCII-armored public key

Tagged releases also publish a signed APT repository to GitHub Pages at:

```text
https://ddv1982.github.io/csv-data-anonymizer/apt/
```

GitHub Pages must be configured to deploy from GitHub Actions. The APT repository should use a dedicated signing key when available:

- `APT_REPO_SIGNING_PRIVATE_KEY`: base64-encoded ASCII-armored GPG private key for APT `Release` metadata
- `APT_REPO_SIGNING_KEY_FINGERPRINT`: full fingerprint for the APT repository signing key
- `APT_REPO_SIGNING_KEY_PASSPHRASE`: passphrase for the APT repository signing key

If the dedicated APT key is not configured yet, the release workflow falls back to the `DEB_SIGNING_*` key so the repository metadata is still signed.

The repository setup package is `csv-anonymizer-repository-setup_1.0_all.deb`. Its version is independent from the app version; keep it at `1.0` unless the repository URL, keyring path, source file path, suite/component, or installed trust contract changes.

## Release Behavior

Pushing a `v*` tag triggers `.github/workflows/release.yml`.

The release workflow:

- validates the tag, package version, and changelog section
- waits for the normal `CI` workflow to succeed on the tagged commit and verifies that it produced the reusable Linux package artifact
- creates or refreshes a draft GitHub Release
- downloads the Linux package artifact from CI, verifies detached GPG signatures, and uploads `AppImage`, `deb`, `rpm`, and `.asc` assets
- validates Linux package AppStream, desktop-file, Debian copyright, and RPM license metadata
- builds a signed static APT repository from the same `.deb`, publishes it to GitHub Pages under `/apt/`, and uploads the repository setup package plus installer script to the draft release
- builds signed and notarized macOS `dmg` and `zip` artifacts for x64 and arm64
- verifies the packaged `.app` with `codesign`, `stapler`, and `spctl`
- uploads the macOS artifacts to the draft release
- publishes the release only after the Linux and macOS jobs succeed

The workflow does not currently sign Windows artifacts or publish RPM/YUM repository metadata. RPM artifacts are uploaded directly to GitHub Releases with detached `.asc` signatures.

## APT Repository Verification

The canonical user install flow is:

```bash
bash <(curl -fsSL https://github.com/ddv1982/csv-data-anonymizer/releases/latest/download/install-apt-repo.sh)
sudo apt update
sudo apt install csv-anonymizer
```

The installer downloads:

```text
https://github.com/ddv1982/csv-data-anonymizer/releases/latest/download/csv-anonymizer-repository-setup_1.0_all.deb
https://github.com/ddv1982/csv-data-anonymizer/releases/latest/download/csv-anonymizer-repository-setup_1.0_all.deb.sha256
https://github.com/ddv1982/csv-data-anonymizer/releases/latest/download/csv-anonymizer-repository-setup_1.0_all.deb.sha256.asc
https://ddv1982.github.io/csv-data-anonymizer/apt/csv-anonymizer-archive-keyring.pgp
```

The release workflow stamps the selected APT signing fingerprint into `scripts/install-apt-repo.sh` before uploading it. The installer validates the downloaded keyring, verifies the signed SHA256 sidecar, checks the setup package hash, then installs the setup package with APT.

After the Pages deploy, verify the hosted repository and release assets:

```bash
curl -fsSI https://ddv1982.github.io/csv-data-anonymizer/apt/dists/stable/InRelease
curl -fsSI https://ddv1982.github.io/csv-data-anonymizer/apt/dists/stable/main/binary-amd64/Packages.gz
curl -fsSI https://ddv1982.github.io/csv-data-anonymizer/apt/dists/stable/main/dep11/Components-amd64.yml.gz
curl -fsSI https://ddv1982.github.io/csv-data-anonymizer/apt/csv-anonymizer-archive-keyring.pgp
curl -fsSI https://github.com/ddv1982/csv-data-anonymizer/releases/latest/download/install-apt-repo.sh
curl -fsSI https://github.com/ddv1982/csv-data-anonymizer/releases/latest/download/csv-anonymizer-repository-setup_1.0_all.deb
curl -fsSI https://github.com/ddv1982/csv-data-anonymizer/releases/latest/download/csv-anonymizer-repository-setup_1.0_all.deb.sha256
curl -fsSI https://github.com/ddv1982/csv-data-anonymizer/releases/latest/download/csv-anonymizer-repository-setup_1.0_all.deb.sha256.asc
```

In a clean Debian/Ubuntu VM, verify the package-manager update path:

```bash
bash <(curl -fsSL https://github.com/ddv1982/csv-data-anonymizer/releases/latest/download/install-apt-repo.sh)
sudo apt update
apt-cache policy csv-anonymizer
sudo apt install csv-anonymizer
```

## Local Validation

Before pushing a release tag, run:

```bash
pnpm install --frozen-lockfile
pnpm run typecheck
pnpm test
pnpm run build
pnpm run release:check -- --expected-tag v1.0.0
```

On macOS, unsigned local packaging can be checked with:

```bash
pnpm run dist:mac:dir
pnpm run smoke:packaged
```

On Linux, check unpacked packaging and the packaged smoke flow with:

```bash
pnpm run dist:linux:dir
xvfb-run --auto-servernum pnpm run smoke:packaged
```

After building Linux release packages, validate package metadata and APT repository generation:

```bash
pnpm run dist:linux
pnpm run linux:metadata:check
pnpm run apt:repo:check
```
