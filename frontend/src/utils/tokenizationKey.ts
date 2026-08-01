export function confirmEphemeralTokenizationKey(key: string | null | undefined) {
  if (!key) return true
  return window.confirm(
    'The repeatable token key is held in memory only and will not be saved by this app. Confirm that you have stored it separately if you need to reproduce these tokens.',
  )
}

export function getTokenizationKeyError(key: string | null | undefined) {
  if (!key) return null
  if (key.length !== 64) return 'Enter exactly 64 hexadecimal characters.'
  if (!/^[0-9a-fA-F]{64}$/.test(key)) return 'Use only hexadecimal characters (0–9 and a–f).'
  return null
}

export function isValidTokenizationKey(key: string | null | undefined) {
  return getTokenizationKeyError(key) === null
}
