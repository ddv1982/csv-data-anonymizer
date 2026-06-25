import { useState } from 'react'
import type { AnonymizeData, PreviewData } from '../types'

export function useWorkflowArtifacts() {
  const [preview, setPreview] = useState<PreviewData | null>(null)
  const [result, setResult] = useState<AnonymizeData | null>(null)

  function clearArtifacts() {
    setPreview(null)
    setResult(null)
  }

  return {
    preview,
    result,
    setPreview,
    setResult,
    clearArtifacts,
  }
}
