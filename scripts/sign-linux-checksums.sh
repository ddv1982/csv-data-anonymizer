#!/usr/bin/env bash
set -euo pipefail

setup_package="dist/rust/artifacts/csv-anonymizer-repository-setup_1.0_all.deb"
artifacts_dir="dist/rust/artifacts"
sign_setup=false
sign_direct=false
dry_run=false

usage() {
  cat <<'USAGE' >&2
Usage: bash scripts/sign-linux-checksums.sh [--setup-package] [--direct-installers] [--dry-run]

Signs Linux release checksum sidecars. The release workflow must provide:
  LINUX_GNUPGHOME
  LINUX_VERIFICATION_GNUPGHOME
  LINUX_GPG_FINGERPRINT
  LINUX_GPG_PASSPHRASE_FILE

--dry-run writes checksum files and skips GPG signing/verification.
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --setup-package)
      sign_setup=true
      shift
      ;;
    --direct-installers)
      sign_direct=true
      shift
      ;;
    --setup-package-path)
      setup_package="$2"
      shift 2
      ;;
    --artifacts-dir)
      artifacts_dir="$2"
      shift 2
      ;;
    --dry-run)
      dry_run=true
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if [ "${sign_setup}" = false ] && [ "${sign_direct}" = false ]; then
  echo "Choose at least one signing target." >&2
  usage
  exit 2
fi

if [ "${dry_run}" = false ]; then
  missing=()
  [ -n "${LINUX_GNUPGHOME:-}" ] || missing+=("LINUX_GNUPGHOME")
  [ -n "${LINUX_VERIFICATION_GNUPGHOME:-}" ] || missing+=("LINUX_VERIFICATION_GNUPGHOME")
  [ -n "${LINUX_GPG_FINGERPRINT:-}" ] || missing+=("LINUX_GPG_FINGERPRINT")
  [ -n "${LINUX_GPG_PASSPHRASE_FILE:-}" ] || missing+=("LINUX_GPG_PASSPHRASE_FILE")

  if [ "${#missing[@]}" -gt 0 ]; then
    echo "Required Linux checksum signing inputs are missing: ${missing[*]}" >&2
    exit 1
  fi
fi

write_checksum() {
  local artifact="$1"
  local checksum="$2"
  local artifact_dir
  local artifact_name
  local checksum_name

  artifact_dir="$(dirname "${artifact}")"
  artifact_name="$(basename "${artifact}")"
  checksum_name="$(basename "${checksum}")"

  if [ ! -f "${artifact}" ]; then
    echo "Expected artifact does not exist: ${artifact}" >&2
    exit 1
  fi

  (
    cd "${artifact_dir}"
    if command -v sha256sum >/dev/null 2>&1; then
      sha256sum "${artifact_name}" > "${checksum_name}"
    else
      shasum -a 256 "${artifact_name}" > "${checksum_name}"
    fi
  )
}

sign_checksum() {
  local artifact="$1"
  local checksum="${artifact}.sha256"
  local signature="${checksum}.asc"

  write_checksum "${artifact}" "${checksum}"

  if [ "${dry_run}" = true ]; then
    printf 'Dry-run: wrote %s and skipped signing %s\n' "${checksum}" "${signature}"
    return
  fi

  GNUPGHOME="${LINUX_GNUPGHOME}" gpg --batch --yes --pinentry-mode loopback \
    --passphrase-file "${LINUX_GPG_PASSPHRASE_FILE}" \
    --local-user "${LINUX_GPG_FINGERPRINT}" --armor --detach-sign \
    --output "${signature}" "${checksum}"
  GNUPGHOME="${LINUX_VERIFICATION_GNUPGHOME}" gpg --quiet --batch --trust-model always --verify "${signature}" "${checksum}"
}

if [ "${sign_setup}" = true ]; then
  sign_checksum "${setup_package}"
fi

if [ "${sign_direct}" = true ]; then
  shopt -s nullglob
  artifacts=(
    "${artifacts_dir}"/csv-anonymizer-*.rpm
    "${artifacts_dir}"/*.AppImage
    "${artifacts_dir}"/csv-anonymizer_*.deb
  )

  if [ "${#artifacts[@]}" -eq 0 ]; then
    echo "Expected at least one direct Linux installer artifact to sign." >&2
    exit 1
  fi

  for artifact in "${artifacts[@]}"; do
    sign_checksum "${artifact}"
  done
fi
