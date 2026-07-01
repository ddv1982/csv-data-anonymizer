import { useState } from 'react'
import { copyTextToClipboard } from '../utils/clipboard'
import { messageFrom } from '../utils/errors'

type CopyBusyState = 'idle' | 'copying'

export function useCopyOutput({
  isBusy,
  onError,
  setBusy,
}: {
  isBusy: boolean
  onError: (message: string | null) => void
  setBusy: (state: CopyBusyState) => void
}) {
  const [copyStatus, setCopyStatus] = useState<string | null>(null)

  async function copyOutput(output: string | null | undefined) {
    if (!output || isBusy) return
    onError(null)
    setBusy('copying')
    try {
      await copyTextToClipboard(output)
      setCopyStatus('Copied')
    } catch (caught) {
      setCopyStatus(null)
      onError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  return { copyOutput, copyStatus, setCopyStatus }
}
