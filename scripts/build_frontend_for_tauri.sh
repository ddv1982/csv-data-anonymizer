#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
frontend_dist="${repo_root}/frontend/dist"
frontend_index="${frontend_dist}/index.html"

validate_frontend_dist() {
  if [[ ! -f "${frontend_index}" ]]; then
    echo "Frontend dist is missing ${frontend_index}." >&2
    return 1
  fi

  if ! grep -Eq '<link[^>]+href="[^"]*assets/[^"]+\.css"' "${frontend_index}"; then
    echo "Frontend dist index.html does not reference a built CSS asset." >&2
    return 1
  fi

  shopt -s nullglob
  local css_files=("${frontend_dist}"/assets/*.css)
  shopt -u nullglob

  if [[ "${#css_files[@]}" -eq 0 ]]; then
    echo "Frontend dist is missing built CSS files under ${frontend_dist}/assets." >&2
    return 1
  fi

  local css_file
  for css_file in "${css_files[@]}"; do
    if [[ ! -s "${css_file}" ]]; then
      echo "Frontend CSS asset ${css_file} is empty." >&2
      return 1
    fi
  done
}

if [[ "${CSV_ANONYMIZER_USE_PREBUILT_FRONTEND:-}" == "1" ]]; then
  if ! validate_frontend_dist; then
    echo "CSV_ANONYMIZER_USE_PREBUILT_FRONTEND=1, but the prebuilt frontend dist is incomplete." >&2
    echo "Build or download the frontend dist artifact before running cargo tauri build." >&2
    exit 1
  fi

  echo "Using prebuilt frontend dist at ${frontend_dist}."
  exit 0
fi

cd "${repo_root}/frontend"
npm run build

validate_frontend_dist
