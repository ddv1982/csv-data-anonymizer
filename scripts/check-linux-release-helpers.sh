#!/usr/bin/env bash
set -euo pipefail

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "${tmp_dir}"
}
trap cleanup EXIT

artifacts_dir="${tmp_dir}/dist/rust/artifacts"
pages_dir="${tmp_dir}/dist/rust/apt-pages"
fingerprint="0123456789ABCDEF0123456789ABCDEF01234567"

mkdir -p "${artifacts_dir}" "${pages_dir}/apt"
printf 'repository setup package fixture\n' > "${artifacts_dir}/csv-anonymizer-repository-setup_1.0_all.deb"
printf 'rpm fixture\n' > "${artifacts_dir}/csv-anonymizer-1.0.0-1.x86_64.rpm"
printf 'appimage fixture\n' > "${artifacts_dir}/CSV Anonymizer.AppImage"
printf 'deb fixture\n' > "${artifacts_dir}/csv-anonymizer_1.0.0_amd64.deb"

bash scripts/sign-linux-checksums.sh \
  --setup-package \
  --direct-installers \
  --artifacts-dir "${artifacts_dir}" \
  --setup-package-path "${artifacts_dir}/csv-anonymizer-repository-setup_1.0_all.deb" \
  --dry-run

for expected in \
  "${artifacts_dir}/csv-anonymizer-repository-setup_1.0_all.deb.sha256" \
  "${artifacts_dir}/csv-anonymizer-1.0.0-1.x86_64.rpm.sha256" \
  "${artifacts_dir}/CSV Anonymizer.AppImage.sha256" \
  "${artifacts_dir}/csv-anonymizer_1.0.0_amd64.deb.sha256"; do
  if [ ! -s "${expected}" ]; then
    echo "Expected dry-run checksum was not written: ${expected}" >&2
    exit 1
  fi
done

printf 'signature fixture\n' > "${artifacts_dir}/csv-anonymizer-repository-setup_1.0_all.deb.sha256.asc"

bash scripts/stage-apt-installer-assets.sh \
  --fingerprint "${fingerprint}" \
  --artifacts-dir "${artifacts_dir}" \
  --pages-dir "${pages_dir}" \
  --stage-public

for expected in \
  "${artifacts_dir}/install-apt-repo.sh" \
  "${pages_dir}/install-apt-repo.sh" \
  "${pages_dir}/apt/csv-anonymizer-repository-setup_1.0_all.deb" \
  "${pages_dir}/apt/csv-anonymizer-repository-setup_1.0_all.deb.sha256" \
  "${pages_dir}/apt/csv-anonymizer-repository-setup_1.0_all.deb.sha256.asc"; do
  if [ ! -s "${expected}" ]; then
    echo "Expected staged APT asset was not written: ${expected}" >&2
    exit 1
  fi
done

echo "Linux release helper check passed."
