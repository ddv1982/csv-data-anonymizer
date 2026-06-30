#!/usr/bin/env bash
set -euo pipefail

artifact="$1"

for attempt in 1 2 3; do
  if xcrun notarytool submit "${artifact}" \
    --key "${APPLE_API_KEY_PATH}" \
    --key-id "${APPLE_API_KEY_ID}" \
    --issuer "${APPLE_API_ISSUER}" \
    --wait; then
    exit 0
  fi

  if [ "${attempt}" -lt 3 ]; then
    sleep $((attempt * 60))
  fi
done

exit 1
