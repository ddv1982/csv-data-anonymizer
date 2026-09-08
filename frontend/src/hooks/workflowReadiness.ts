import { isValidTokenizationKey } from '../utils/tokenizationKey'

type LocalAiReadiness = { ready: boolean; downloadRunning: boolean }
export type ProtectionBlocker = 'localAi' | 'preparedAnalysis' | 'tokenizationKey'
export type ProtectionReadinessInput = {
  usesLocalAi: boolean
  localAi: LocalAiReadiness
  requiresPreparedAnalysis: boolean
  hasPreparedAnalysis: boolean
  usesTokenization: boolean
  tokenizationKey: string | null | undefined
}

export function isSmartReplacementBlocked(usesLocalAi: boolean, localAi: LocalAiReadiness): boolean {
  return usesLocalAi && (!localAi.ready || localAi.downloadRunning)
}

export function getProtectionReadiness(input: ProtectionReadinessInput): {
  blocker: ProtectionBlocker | null
  tokenizationKey: string | null
  localAiBlocked: boolean
} {
  const localAiBlocked = isSmartReplacementBlocked(input.usesLocalAi, input.localAi)
  const tokenizationKey = input.usesTokenization ? input.tokenizationKey ?? null : null
  let blocker: ProtectionBlocker | null = null
  if (localAiBlocked) blocker = 'localAi'
  else if (input.requiresPreparedAnalysis && !input.hasPreparedAnalysis) blocker = 'preparedAnalysis'
  else if (!isValidTokenizationKey(tokenizationKey)) blocker = 'tokenizationKey'
  return { blocker, tokenizationKey, localAiBlocked }
}
