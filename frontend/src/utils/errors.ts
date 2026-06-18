export function messageFrom(value: unknown) {
  if (value instanceof Error) return value.message
  if (typeof value === 'string') return value
  return 'Unexpected application error.'
}
