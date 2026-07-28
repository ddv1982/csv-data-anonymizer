export function directoryOf(path: string) {
  const slashIndex = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  if (slashIndex < 0) return null
  if (slashIndex === 0) return path[0]
  if (slashIndex === 2 && /^[A-Za-z]:[\\/]/.test(path)) return path.slice(0, 3)
  return path.slice(0, slashIndex)
}

export function defaultOutputPathWithSuffix(inputPath: string, suffix: string) {
  const normalizedSuffix = suffix.trim() || '_private_output'
  const slashIndex = Math.max(inputPath.lastIndexOf('/'), inputPath.lastIndexOf('\\'))
  const directory = slashIndex >= 0 ? inputPath.slice(0, slashIndex + 1) : ''
  const fileName = slashIndex >= 0 ? inputPath.slice(slashIndex + 1) : inputPath
  const dotIndex = fileName.lastIndexOf('.')

  if (dotIndex > 0) {
    return `${directory}${fileName.slice(0, dotIndex)}${normalizedSuffix}${fileName.slice(dotIndex)}`
  }

  return `${directory}${fileName}${normalizedSuffix}`
}
