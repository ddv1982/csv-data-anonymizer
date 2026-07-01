export const glossaryTerms = {
  standardMasking: {
    title: 'Standard CSV transform',
    body: 'Transforms selected cells in the original row-level CSV and leaves unselected columns unchanged. It is useful local masking and pseudonymization, not a formal anonymity guarantee.',
  },
  typeOverride: {
    title: 'Type override',
    body: 'Manual correction when automatic detection picked the wrong data type.',
  },
  strategy: {
    title: 'Strategy',
    body: 'How selected cell values are transformed in standard row-level mode. Auto and Pseudonymize use type-based rules; Mask, Redact, Tokenize, Smart replacement, and Pass through are explicit choices.',
  },
  role: {
    title: 'Column category',
    body: 'The detector category used in the report to explain why a column was considered identifying or sensitive.',
  },
  risk: {
    title: 'Risk',
    body: 'Estimated likelihood that a column contains personal or identifying data.',
  },
  pii: {
    title: 'PII',
    body: 'Personally identifiable information: data that can identify a person directly or indirectly.',
  },
  directIdentifier: {
    title: 'Direct ID',
    body: 'A value that can identify a person by itself, such as an email, phone, name, tax ID, or address.',
  },
  quasiIdentifier: {
    title: 'Quasi-ID',
    body: 'A value that may identify someone when combined with other columns, such as timestamp, postal code, country, IP, or UUID.',
  },
  sensitive: {
    title: 'Sensitive',
    body: 'Private information to protect, such as income, health, status, or other confidential attributes.',
  },
  attribute: {
    title: 'Attribute',
    body: 'Regular data kept for utility and not treated as an identifier.',
  },
  exclude: {
    title: 'Exclude',
    body: 'A column intentionally left out of protection because it is not useful, not sensitive, or should stay unchanged.',
  },
  pseudonymize: {
    title: 'Pseudonymize',
    body: 'Use type-based replacement rules to produce fake or shape-preserving values when possible. Repeated source values stay consistent within the current run. Some low-risk types may stay unchanged.',
  },
  tokenize: {
    title: 'Tokenize',
    body: 'Replace values with opaque tok_... tokens that stay consistent within the current run.',
  },
  smartReplacement: {
    title: 'Smart replacement',
    body: 'Use the local AI model to generate context-aware replacements. If no valid AI value is available, the app falls back to rule-based pseudonymization.',
  },
  fallback: {
    title: 'Fallback',
    body: 'Rule-based replacement used when a Smart replacement request cannot produce a valid local AI value.',
  },
  mask: {
    title: 'Mask',
    body: 'Replace every non-space character with an asterisk, preserving spaces and length cues.',
  },
  redact: {
    title: 'Redact',
    body: 'Replace values with typed placeholders such as [EMAIL], [PERSON], or [DATE].',
  },
  passThrough: {
    title: 'Pass through',
    body: 'Keep the original value unchanged.',
  },
  localAi: {
    title: 'Local AI',
    body: 'AI running through Ollama on this device over localhost, not a cloud API.',
  },
  ollama: {
    title: 'Ollama',
    body: 'Local runtime used to run downloaded AI models on this computer.',
  },
  model: {
    title: 'Model',
    body: 'The downloaded AI model used to generate smart replacement values.',
  },
  localhost: {
    title: 'localhost',
    body: 'A private network address for this computer.',
  },
  gemma: {
    title: 'Gemma 3 4B',
    body: 'The default lightweight local model suggested for smart replacement.',
  },
  pseudonymizedColumns: {
    title: 'Pseudonymized columns',
    body: 'Columns counted by the run report as using Auto or Pseudonymize rule-based replacement behavior.',
  },
  smartReplacementColumns: {
    title: 'Smart replacement columns',
    body: 'Selected columns transformed with Local AI smart replacement.',
  },
  opaqueTokenColumns: {
    title: 'Opaque token columns',
    body: 'Columns transformed into opaque tok_... values.',
  },
  maskedColumns: {
    title: 'Masked columns',
    body: 'Columns whose selected values were masked with asterisk characters.',
  },
  redactedColumns: {
    title: 'Redacted columns',
    body: 'Columns whose selected values were replaced with typed placeholders such as [EMAIL], [PERSON], or [DATE].',
  },
  passThroughNoOp: {
    title: 'Pass-through/no-op',
    body: 'Selected columns whose original values were kept unchanged or currently have no transformation.',
  },
  uniquePseudonyms: {
    title: 'Unique pseudonyms',
    body: 'Distinct replacement values created for source values during the run.',
  },
  opaqueTokenValues: {
    title: 'Opaque token values',
    body: 'Distinct tok_... replacement values created during the run.',
  },
  repeatedSourceReuses: {
    title: 'Repeated source reuses',
    body: 'Times a repeated source value reused its existing replacement for consistency.',
  },
  collisionsAvoided: {
    title: 'Collisions avoided',
    body: 'Times the app avoided assigning the same readable replacement to different source values.',
  },
  poolExhaustions: {
    title: 'Pool exhaustions',
    body: 'Times a finite replacement pool ran out and generated fallback values were used.',
  },
  smartReplacementValues: {
    title: 'Smart replacement values',
    body: 'Replacement values returned by the local AI workflow.',
  },
  smartFallbacks: {
    title: 'Smart fallbacks',
    body: 'AI replacement values that were missing or invalid and fell back to rule-based pseudonymization.',
  },
  smartRejections: {
    title: 'Smart rejections',
    body: 'Local AI replacement candidates rejected during validation before rule-based fallback handling.',
  },
  formatFallbacks: {
    title: 'Format fallbacks',
    body: 'Values that did not match their column’s detected format and were replaced with generic pseudonyms instead of format-preserving ones.',
  },
} as const

export type GlossaryKey = keyof typeof glossaryTerms
