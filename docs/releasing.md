# Release Workflow

Use this workflow when publishing downloadable CSV Anonymizer desktop artifacts.

## Version And Tag

Update `package.json` and add release notes to `CHANGELOG.md` using this heading format:

```md
## v1.0.0 - YYYY-MM-DD
- Release note
```

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

The Debian package is GPG-signed on `main` pushes before CI uploads the reusable Linux artifact bundle. Configure these GitHub Actions inputs before pushing a release tag:

- `DEB_SIGNING_PRIVATE_KEY`: base64-encoded ASCII-armored GPG private key
- `DEB_SIGNING_KEY_FINGERPRINT`: full fingerprint for the Debian signing key
- `DEB_SIGNING_KEY_PASSPHRASE`: passphrase for the Debian signing key
- `DEB_SIGNING_PUBLIC_KEY`: repository variable containing the base64-encoded ASCII-armored public key

`APT_REPO_SIGNING_*` secrets may also be configured for a future hosted APT repository, but the current Electron release workflow publishes direct GitHub Release assets only.

## Release Behavior

Pushing a `v*` tag triggers `.github/workflows/release.yml`.

The release workflow:

- validates the tag, package version, and changelog section
- waits for the normal `CI` workflow to succeed on the tagged commit and verifies that it produced the reusable Linux package artifact
- creates or refreshes a draft GitHub Release
- downloads the Linux package artifact from CI, verifies Debian package signatures, and uploads `AppImage`, `deb`, and `rpm` assets
- builds signed and notarized macOS `dmg` and `zip` artifacts for x64 and arm64
- verifies the packaged `.app` with `codesign`, `stapler`, and `spctl`
- uploads the macOS artifacts to the draft release
- publishes the release only after the Linux and macOS jobs succeed

The workflow does not currently sign Windows artifacts or publish a hosted Linux package repository. Add those as separate distribution steps when those channels are needed.

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
