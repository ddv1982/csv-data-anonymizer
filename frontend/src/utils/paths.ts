export function directoryOf(path: string) {
  const slashIndex = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return slashIndex > 0 ? path.slice(0, slashIndex) : null
}
