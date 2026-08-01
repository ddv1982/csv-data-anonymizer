import { useState } from 'react'
import { generateQuickValues } from '../tauri'
import type { AnonymizationStrategy, DataType, QuickTransformData } from '../types'
import { messageFrom } from '../utils/errors'
import { confirmEphemeralTokenizationKey, isValidTokenizationKey } from '../utils/tokenizationKey'
import { useCopyOutput } from './useCopyOutput'
import type { LocalAiState } from './useLocalAi'

export type QuickBusyState = 'idle' | 'generating' | 'copying'

export const QUICK_MIN_COUNT = 1
export const QUICK_MAX_COUNT = 1000

type QuickGenerateWorkflowOptions = {
  settingsLoaded: boolean
  localAi: LocalAiState
  onError: (message: string | null) => void
  tokenizationKey?: string | null
}

/**
 * The generator behind the "Quick by Data Type" tab.
 *
 * Lifted out of the view so `App` can read `isBusy` during its own render. The view
 * used to mirror this flag up through an effect, which meant the tab strip decided
 * whether to disable itself from a value that only arrived after the next paint.
 */
export function useQuickGenerateWorkflow({
  settingsLoaded,
  localAi,
  onError,
  tokenizationKey = null,
}: QuickGenerateWorkflowOptions) {
  const [dataType, setDataTypeState] = useState<DataType>('email')
  const [strategy, setStrategyState] = useState<AnonymizationStrategy>('auto')
  const [count, setCountState] = useState(1)
  const [result, setResult] = useState<QuickTransformData | null>(null)
  const [busy, setBusy] = useState<QuickBusyState>('idle')

  const isBusy = busy !== 'idle'
  const { copyOutput, copyStatus, setCopyStatus } = useCopyOutput({ isBusy, onError, setBusy })
  const usesLocalAi = strategy === 'localAi'
  const usesTokenization = strategy === 'tokenize'
  const activeTokenizationKey = usesTokenization ? tokenizationKey : null
  const localAiBlocked = usesLocalAi && (!localAi.ready || localAi.downloadRunning)
  const canGenerate =
    settingsLoaded && count >= QUICK_MIN_COUNT && count <= QUICK_MAX_COUNT && !isBusy && !localAiBlocked &&
    isValidTokenizationKey(activeTokenizationKey)

  /** Any input change invalidates the values on screen: they were generated for the old settings. */
  function clearOutput() {
    setResult(null)
    setCopyStatus(null)
  }

  function setDataType(nextDataType: DataType) {
    setDataTypeState(nextDataType)
    clearOutput()
  }

  function setStrategy(nextStrategy: AnonymizationStrategy) {
    setStrategyState(nextStrategy)
    clearOutput()
  }

  function setCount(nextCount: number) {
    setCountState(nextCount)
    clearOutput()
  }

  async function generate() {
    if (!settingsLoaded || count < QUICK_MIN_COUNT || count > QUICK_MAX_COUNT || isBusy) return
    if (localAiBlocked) {
      onError('Set up Local AI before generating Smart replacement values.')
      return
    }
    if (!isValidTokenizationKey(activeTokenizationKey)) {
      onError('Enter a valid 64-character hexadecimal tokenization key before generating values.')
      return
    }
    if (!confirmEphemeralTokenizationKey(activeTokenizationKey)) return
    onError(null)
    setBusy('generating')
    setCopyStatus(null)
    try {
      const generated = await generateQuickValues({
        dataType,
        strategy,
        count,
        localAi: localAi.request,
        tokenizationKey: activeTokenizationKey,
      })
      setResult(generated)
    } catch (caught) {
      onError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  return {
    dataType,
    strategy,
    count,
    result,
    busy,
    isBusy,
    copyStatus,
    usesLocalAi,
    localAiBlocked,
    canGenerate,
    setDataType,
    setStrategy,
    setCount,
    generate,
    copyOutput: () => copyOutput(result?.output),
  }
}

export type QuickGenerateWorkflowState = ReturnType<typeof useQuickGenerateWorkflow>
