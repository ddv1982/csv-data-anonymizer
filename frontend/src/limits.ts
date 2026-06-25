export const MAX_PASTE_CONTENT_BYTES = 5 * 1024 * 1024

export function byteLength(value: string) {
  return new TextEncoder().encode(value).length
}

export function formatByteLimit(bytes: number) {
  const mib = bytes / (1024 * 1024)
  return mib >= 1 ? `${mib.toFixed(0)} MiB` : `${bytes.toLocaleString()} bytes`
}
