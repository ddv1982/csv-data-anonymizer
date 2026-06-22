export const glossaryTerms = {
  releaseMode: {
    title: 'Privacy release mode',
    body: 'The output workflow: standard row-level transformation uses column strategies, while k/l/t tabular, DP aggregate, and Synthetic data use privacy roles and release settings.',
  },
  standardMasking: {
    title: 'Standard CSV transform',
    body: 'Transforms selected cells in the original row-level CSV and leaves unselected columns unchanged. It is useful local masking and pseudonymization, not a formal anonymity guarantee.',
  },
  formalTabular: {
    title: 'k/l/t tabular',
    body: 'A row-level release that redacts Direct IDs, generalizes Quasi-IDs, and checks k-anonymity plus optional l-diversity and t-closeness. Sensitive values are checked, not masked.',
  },
  dpAggregate: {
    title: 'DP aggregate',
    body: 'Writes noisy count, sum, or mean results instead of row-level source rows. Repeated releases spend more privacy budget; repeatable deterministic output is not available for this mode.',
  },
  syntheticData: {
    title: 'Synthetic data',
    body: 'Generates sampled test data from simple per-column distributions. It does not preserve row relationships or make a DP synthetic guarantee.',
  },
  kAnonymity: {
    title: 'k-anonymity',
    body: 'Minimum size for each released equivalence class, which is a group of rows with the same quasi-identifier pattern.',
  },
  lDiversity: {
    title: 'l-diversity',
    body: 'Optional check requiring enough distinct sensitive values in each released equivalence class. It needs at least one Sensitive column role.',
  },
  tCloseness: {
    title: 't-closeness',
    body: "Optional categorical distance check that limits how different each class's sensitive-value distribution can be from the dataset. It needs at least one Sensitive column role.",
  },
  suppressSmallClasses: {
    title: 'Suppress small classes',
    body: 'When enabled, drops rows whose equivalence class is smaller than k in formal tabular mode.',
  },
  epsilon: {
    title: 'Epsilon',
    body: 'Differential privacy budget. Smaller values add more noise and protect privacy more; larger values keep aggregates closer to source data.',
  },
  syntheticEpsilon: {
    title: 'Unsupported synthetic epsilon',
    body: 'A stale setting from older configs. This simple generator does not support DP synthetic data, so synthetic output must be created without epsilon.',
  },
  syntheticRowCount: {
    title: 'Synthetic row count',
    body: 'Number of generated rows to write. Leave empty to match the source data row count. The app caps this at 1,000,000 rows.',
  },
  aggregate: {
    title: 'Aggregate',
    body: 'The summary statistic to release: count, sum, or mean.',
  },
  groupColumn: {
    title: 'Group column',
    body: 'Optional Attribute-role column used to split aggregate results into public allowed group values before noise is added.',
  },
  publicGroupLabels: {
    title: 'Public group labels',
    body: 'Confirms that grouped DP output may publish the configured group labels. The group column must be safe to release as an Attribute.',
  },
  publicGroupDomain: {
    title: 'Allowed group values',
    body: 'The complete public set of group labels to release. The app writes every configured group and rejects source rows outside these values.',
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
  privacyUnitColumn: {
    title: 'Privacy unit',
    body: 'Optional column identifying the person, device, account, or other unit protected by contribution bounding.',
  },
  maxContributionsPerUnit: {
    title: 'Max contributions',
    body: 'Maximum source rows one privacy unit may contribute to a DP aggregate release. Extra rows are skipped before aggregation.',
  },
  dpBudget: {
    title: 'DP budget tracking',
    body: 'Local release history for DP aggregate releases. It uses basic linear composition and is only as complete as releases recorded in this app.',
  },
  dpBudgetLimit: {
    title: 'DP budget limit',
    body: 'The local cumulative epsilon cap for DP aggregate releases.',
  },
  dpBudgetSpent: {
    title: 'Spent epsilon',
    body: 'Backend-recorded cumulative epsilon for this app. Settings show current spent epsilon; reports show before and after values for a release.',
  },
  dpBudgetRemaining: {
    title: 'Remaining epsilon',
    body: 'Budget limit minus cumulative spent epsilon after the release. Negative values mean the release is over budget.',
  },
  dpBudgetStatus: {
    title: 'DP budget status',
    body: 'Whether the DP aggregate release is within, exactly at, or over the configured local budget limit.',
  },
  dpBudgetAction: {
    title: 'Over-budget behavior',
    body: 'Block prevents a DP aggregate release that would exceed the budget. Warn only records the over-budget release and allows it.',
  },
  typeOverride: {
    title: 'Type override',
    body: 'Manual correction when automatic detection picked the wrong data type.',
  },
  strategy: {
    title: 'Strategy',
    body: 'How selected cell values are transformed in standard row-level mode. Auto and Pseudonymize use type-based rules; Mask, Tokenize, Smart replacement, and Pass through are explicit choices.',
  },
  role: {
    title: 'Role',
    body: 'How privacy release modes treat this column. Auto infers Direct ID, Quasi-ID, or Attribute, but not Sensitive.',
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
    body: 'Private information to protect, such as income, health, status, or other confidential attributes. Needed for l-diversity and t-closeness checks.',
  },
  attribute: {
    title: 'Attribute',
    body: 'Regular data kept for utility and not treated as an identifier by formal privacy modes.',
  },
  exclude: {
    title: 'Exclude',
    body: 'Formal mode blanks this column. Synthetic mode writes generated identifier-like placeholders for it.',
  },
  pseudonymize: {
    title: 'Pseudonymize',
    body: 'Use type-based replacement rules to produce consistent fake or shape-preserving values when possible. Some low-risk types may stay unchanged.',
  },
  tokenize: {
    title: 'Tokenize',
    body: 'Replace values with stable opaque tok_... tokens.',
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
