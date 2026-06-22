export const glossaryTerms = {
  releaseMode: {
    title: 'Privacy release mode',
    body: 'The output style the app will write: row-level masking, formal tabular release, DP aggregate, or synthetic rows.',
  },
  standardMasking: {
    title: 'Standard masking',
    body: 'Transforms selected cells in the original row-level file. It is local masking and pseudonymization, not a formal anonymity guarantee.',
  },
  formalTabular: {
    title: 'k/l/t tabular',
    body: 'A row-level release that redacts direct identifiers, generalizes quasi-identifiers, and checks k-anonymity plus optional l-diversity and t-closeness.',
  },
  dpAggregate: {
    title: 'DP aggregate',
    body: 'Writes noisy count, sum, or mean results instead of row-level source rows.',
  },
  syntheticData: {
    title: 'Synthetic data',
    body: 'Generates new rows from simple per-column distributions. Direct identifiers are replaced; this MVP does not make a DP synthetic guarantee.',
  },
  kAnonymity: {
    title: 'k-anonymity',
    body: 'Minimum size for each released equivalence class, which is a group of rows with the same quasi-identifier pattern.',
  },
  lDiversity: {
    title: 'l-diversity',
    body: 'Optional check requiring enough distinct sensitive values in each released equivalence class.',
  },
  tCloseness: {
    title: 't-closeness',
    body: "Optional categorical distance check that limits how different each class's sensitive-value distribution can be from the dataset.",
  },
  suppressSmallClasses: {
    title: 'Suppress small classes',
    body: 'Drops rows whose equivalence class is smaller than k when formal tabular mode runs.',
  },
  epsilon: {
    title: 'Epsilon',
    body: 'Differential privacy budget. Smaller values add more noise and protect privacy more; larger values keep aggregates closer to source data.',
  },
  syntheticEpsilon: {
    title: 'Synthetic DP epsilon',
    body: 'Records a requested synthetic-data privacy budget. The current MVP reports that a DP synthesizer is not implemented.',
  },
  aggregate: {
    title: 'Aggregate',
    body: 'The summary statistic to release: count, sum, or mean.',
  },
  groupColumn: {
    title: 'Group column',
    body: 'Optional column used to split aggregate results into groups before noise is added.',
  },
  valueColumn: {
    title: 'Value column',
    body: 'Numeric column used for sum or mean releases. Count releases do not need it.',
  },
  lowerBound: {
    title: 'Lower bound',
    body: 'Public minimum used to clamp numeric values before a noisy sum or mean release.',
  },
  upperBound: {
    title: 'Upper bound',
    body: 'Public maximum used to clamp numeric values before a noisy sum or mean release.',
  },
  typeOverride: {
    title: 'Type override',
    body: 'Manual correction when automatic detection picked the wrong data type.',
  },
  strategy: {
    title: 'Strategy',
    body: 'How selected cell values are transformed in standard masking mode.',
  },
  role: {
    title: 'Role',
    body: 'How formal privacy release modes treat this column.',
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
    body: 'Regular data kept for utility and not treated as an identifier by formal privacy modes.',
  },
  exclude: {
    title: 'Exclude',
    body: 'Marks the column for excluded release behavior. Formal mode blanks it; synthetic mode writes generated identifier-like placeholders.',
  },
  pseudonymize: {
    title: 'Pseudonymize',
    body: 'Replace each source value with a consistent fake or shape-preserving value when possible.',
  },
  tokenize: {
    title: 'Tokenize',
    body: 'Replace values with stable opaque tok_... tokens.',
  },
  smartReplacement: {
    title: 'Smart replacement',
    body: 'Use the local AI model to generate context-aware replacements. If no valid AI value is available, the app falls back to rule-based pseudonymization.',
  },
  mask: {
    title: 'Mask',
    body: 'Replace every non-space character with an asterisk.',
  },
  passThrough: {
    title: 'Pass through',
    body: 'Keep the original value unchanged.',
  },
  localAi: {
    title: 'Local AI',
    body: 'AI running on this device through the local runtime, not a cloud API.',
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
    body: 'Selected columns transformed by Auto or Pseudonymize using rule-based replacements.',
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
  generalizedColumns: {
    title: 'Generalized columns',
    body: 'Quasi-identifier columns made less precise by formal tabular release.',
  },
  passThroughNoOp: {
    title: 'Pass-through/no-op',
    body: 'Selected columns whose original values were kept unchanged or currently have no transformation.',
  },
  suppressedRows: {
    title: 'Suppressed rows',
    body: 'Rows not written because they fell below the configured formal privacy threshold.',
  },
  syntheticRows: {
    title: 'Synthetic rows',
    body: 'Generated rows written by synthetic data mode.',
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
  formalModel: {
    title: 'Formal model',
    body: 'A privacy check reported by a formal, DP aggregate, or synthetic release mode.',
  },
} as const

export type GlossaryKey = keyof typeof glossaryTerms
