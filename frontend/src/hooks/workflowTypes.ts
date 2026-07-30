import type { Dispatch, SetStateAction } from 'react'
import type { AnonymizeData, AppSettings } from '../types'
import type { LocalAiState } from './useLocalAi'

export type BusyState = 'idle' | 'picking' | 'loading' | 'preview' | 'running'

/**
 * The state every CSV sub-hook needs and none of them owns.
 *
 * Built once by `useAnonymizerWorkflow` and handed down whole. Passing the six
 * members individually meant each sub-hook re-declared them in its own options type
 * and the parent wired them by name three times over, so adding one member was a
 * four-file edit.
 */
export type WorkflowShell = {
  busy: BusyState
  setBusy: Dispatch<SetStateAction<BusyState>>
  setError: Dispatch<SetStateAction<string | null>>
  setResult: Dispatch<SetStateAction<AnonymizeData | null>>
  settings: AppSettings
  localAi: LocalAiState
}
