export function messageFrom(value: unknown) {
  if (value instanceof Error) return sanitizeErrorMessage(value.message)
  if (typeof value === 'string') return sanitizeErrorMessage(value)
  return 'Unexpected application error.'
}

function sanitizeErrorMessage(message: string) {
  const trimmed = message.trim()
  if (!trimmed) return 'Unexpected application error.'

  const lower = trimmed.toLowerCase()
  if (
    lower.includes('panicked at') ||
    lower.includes('backtrace') ||
    /\bthread\s+'.*'\s+panicked\b/i.test(trimmed) ||
    /\bsrc\/[^\s]+\.rs:\d+/i.test(trimmed)
  ) {
    return 'Unexpected application error.'
  }

  if (lower.includes('already exists')) {
    return 'Output file already exists. Choose a different path or enable overwrite.'
  }
  if (
    lower.includes('permission denied') ||
    lower.includes('access is denied') ||
    lower.includes('operation not permitted')
  ) {
    return 'The app does not have permission to access that file or folder.'
  }
  if (lower.includes('no such file') || lower.includes('not found') || lower.includes('does not exist')) {
    return 'The selected file could not be found. Check the path and try again.'
  }
  if (lower.includes('is a directory') || lower.includes('not a file')) {
    return 'Choose a CSV file instead of a folder.'
  }
  if (
    lower.includes('failed to open') ||
    lower.includes('could not open') ||
    lower.includes('failed to read') ||
    lower.includes('could not read') ||
    lower.includes('failed to write') ||
    lower.includes('could not write') ||
    lower.includes('i/o error') ||
    /\bos error \d+\b/i.test(trimmed)
  ) {
    return 'The app could not read or write the selected file. Check the path and permissions.'
  }

  return stripSensitiveDetails(trimmed)
}

function stripSensitiveDetails(message: string) {
  return message
    .replace(/(^|[\s(["'])\/(?:Users|home|private|var|tmp|Volumes|Applications|Library|System|opt|etc)\/[^\s"',)]+/g, '$1[path]')
    .replace(/(^|[\s(["'])[A-Za-z]:\\[^\s"',)]+/g, '$1[path]')
    .replace(/\bat\s+[^\s]+\.rs:\d+(?::\d+)?/g, 'at [internal]')
}
