#!/usr/bin/env bash
set -euo pipefail

artifact="$1"

# Retry only transient notarytool failures (network errors, timeouts,
# in-progress polling issues). A terminal "Invalid"/"Rejected" verdict is
# deterministic - resubmitting the same artifact cannot succeed - so fail fast.
for attempt in 1 2 3; do
  set +e
  output="$(xcrun notarytool submit "${artifact}" \
    --key "${APPLE_API_KEY_PATH}" \
    --key-id "${APPLE_API_KEY_ID}" \
    --issuer "${APPLE_API_ISSUER}" \
    --wait 2>&1)"
  status=$?
  set -e
  printf '%s\n' "${output}"

  if printf '%s\n' "${output}" | grep -qiE 'status: (Invalid|Rejected)'; then
    echo "Notarization returned a terminal verdict for ${artifact}; not retrying." >&2
    echo "Fetch the log with: xcrun notarytool log <submission-id>" >&2
    exit 1
  fi

  if [ "${status}" -eq 0 ]; then
    exit 0
  fi

  if [ "${attempt}" -lt 3 ]; then
    echo "notarytool failed transiently (exit ${status}); retrying in $((attempt * 60))s..." >&2
    sleep $((attempt * 60))
  fi
done

echo "notarytool failed after 3 attempts for ${artifact}." >&2
exit 1
