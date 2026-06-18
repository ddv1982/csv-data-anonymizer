#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
frontend_dist="${repo_root}/frontend/dist"
frontend_index="${frontend_dist}/index.html"

if [[ "${CSV_ANONYMIZER_USE_PREBUILT_FRONTEND:-}" == "1" ]]; then
  if [[ ! -f "${frontend_index}" ]]; then
    echo "CSV_ANONYMIZER_USE_PREBUILT_FRONTEND=1, but ${frontend_index} is missing." >&2
    echo "Build or download the frontend dist artifact before running cargo tauri build." >&2
    exit 1
  fi

  echo "Using prebuilt frontend dist at ${frontend_dist}."
  exit 0
fi

cd "${repo_root}/frontend"
npm run build
