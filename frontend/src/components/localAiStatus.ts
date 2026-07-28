export function localAiStatus(enabled: boolean, downloading: boolean, ready: boolean, hasStatus: boolean) {
  if (!enabled) return { label: 'Off', ready: false }
  if (downloading) return { label: 'Downloading', ready: false }
  if (ready) return { label: 'Ready', ready: true }
  if (hasStatus) return { label: 'Setup needed', ready: false }
  return { label: 'Checking', ready: false }
}
