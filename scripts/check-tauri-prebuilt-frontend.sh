#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

temp_dir="$(mktemp -d)"
css_file=""
css_name=""

cleanup() {
  if [[ -n "${css_file}" && -n "${css_name}" && -f "${temp_dir}/css/${css_name}" ]]; then
    mkdir -p "$(dirname "${css_file}")"
    mv "${temp_dir}/css/${css_name}" "${css_file}"
  fi
  if [[ -d "${temp_dir}/dist" ]]; then
    rm -rf frontend/dist
    mv "${temp_dir}/dist" frontend/dist
  fi
  rm -rf "${temp_dir}"
}
trap cleanup EXIT

if [[ ! -d frontend/dist ]]; then
  echo "Expected frontend/dist to exist. Run npm run frontend:build first." >&2
  exit 1
fi

mv frontend/dist "${temp_dir}/dist"
if CSV_ANONYMIZER_USE_PREBUILT_FRONTEND=1 scripts/build_frontend_for_tauri.sh; then
  echo "Expected prebuilt frontend mode to fail without frontend/dist/index.html." >&2
  exit 1
fi

mv "${temp_dir}/dist" frontend/dist
css_file="$(find frontend/dist/assets -maxdepth 1 -type f -name '*.css' | head -n 1)"
if [[ -z "${css_file}" ]]; then
  echo "Expected frontend build to emit a CSS asset under frontend/dist/assets." >&2
  exit 1
fi

mkdir -p "${temp_dir}/css"
css_name="$(basename "${css_file}")"
mv "${css_file}" "${temp_dir}/css/${css_name}"
if CSV_ANONYMIZER_USE_PREBUILT_FRONTEND=1 scripts/build_frontend_for_tauri.sh; then
  echo "Expected prebuilt frontend mode to fail without a built CSS asset." >&2
  exit 1
fi

mv "${temp_dir}/css/${css_name}" "${css_file}"
css_file=""
css_name=""
CSV_ANONYMIZER_USE_PREBUILT_FRONTEND=1 scripts/build_frontend_for_tauri.sh
