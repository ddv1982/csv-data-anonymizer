import { useEffect, useRef, useState } from 'react'
import { generateQuickValues } from '../tauri'
import type { AnonymizationStrategy, DataType, QuickTransformData } from '../types'
import { messageFrom } from '../utils/errors'
import { confirmEphemeralTokenizationKey } from '../utils/tokenizationKey'
import { useCopyOutput } from './useCopyOutput'
import type { LocalAiState } from './useLocalAi'
import { getProtectionReadiness } from './workflowReadiness'
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
 * Lives in `App` so the tab strip can read `isBusy` during the same render.
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
  const getGenerateReadiness = () => {
    const protection = getProtectionReadiness({
      usesLocalAi,
      localAi,
      requiresPreparedAnalysis: false,
      hasPreparedAnalysis: true,
      usesTokenization,
      tokenizationKey,
    })
    const blocker = !settingsLoaded || count < QUICK_MIN_COUNT || count > QUICK_MAX_COUNT || isBusy
      ? 'unavailable'
      : protection.blocker
    return { blocker, protection }
  }
  const readiness = getGenerateReadiness()
  const activeTokenizationKey = readiness.protection.tokenizationKey
  const localAiBlocked = readiness.protection.localAiBlocked
  const canGenerate = readiness.blocker === null

  /** Any input change invalidates the values on screen: they were generated for the old settings. */
  function clearOutput() {
    setResult(null)
    setCopyStatus(null)
  }
  const previousTokenizationKey = useRef(activeTokenizationKey)

  useEffect(() => {
    if (previousTokenizationKey.current === activeTokenizationKey) return
    previousTokenizationKey.current = activeTokenizationKey
    setResult(null)
    setCopyStatus(null)
  }, [activeTokenizationKey, setCopyStatus, setResult])

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
    const current = getGenerateReadiness()
    if (current.blocker === 'unavailable') return
    if (current.blocker === 'localAi') {
      onError('Set up Local AI before generating Smart replacement values.')
      return
    }
    if (current.blocker === 'tokenizationKey') {
      onError('Enter a valid 64-character hexadecimal tokenization key before generating values.')
      return
    }
    if (!confirmEphemeralTokenizationKey(current.protection.tokenizationKey)) return
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
