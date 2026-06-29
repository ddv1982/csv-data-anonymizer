import type { GlossaryKey } from './glossary'

type HelpTextSegment = string | { text: string; term: GlossaryKey }
type HelpText = string | readonly HelpTextSegment[]
type SectionHelpPoint = {
  label: string
  text: HelpText
}
type SectionHelpEntry = {
  title: string
  summary: readonly HelpText[]
  points: readonly SectionHelpPoint[]
}

export const sectionHelp = {
  selectFile: {
    title: 'Select File',
    summary: [
      'Start by choosing the CSV you want to transform. The app reads the header row and a sample of rows so it can detect column types, risk, and suggested settings before you write any output.',
    ],
    points: [
      {
        label: 'What changes',
        text: 'Nothing is written at this step. Browse or type a path, then the app loads metadata from the source file.',
      },
      {
        label: 'What to check',
        text: 'Make sure the file path points to the source CSV and that the detected row count and columns look like the file you intended to use.',
      },
    ],
  },
  selectColumns: {
    title: 'Select Data to Transform',
    summary: [
      [
        'Choose which columns need protection, then adjust the detected type, ',
        { text: 'Strategy', term: 'strategy' },
        ', and ',
        { text: 'Role', term: 'role' },
        ' where the automatic guess is not enough.',
      ],
    ],
    points: [
      {
        label: 'Strategy',
        text: [
          'Used by standard row-level transformation. Auto and ',
          { text: 'Pseudonymize', term: 'pseudonymize' },
          ' run type-based replacements; some types such as booleans, country codes, percentages, currency, and enums can stay unchanged. ',
          { text: 'Mask', term: 'mask' },
          ' replaces every non-space character with *, ',
          { text: 'Tokenize', term: 'tokenize' },
          ' writes stable tok_ values, ',
          { text: 'Smart replacement', term: 'smartReplacement' },
          ' uses ',
          { text: 'Local AI', term: 'localAi' },
          ', and ',
          { text: 'Pass through', term: 'passThrough' },
          ' keeps the original value.',
        ],
      },
      {
        label: 'Role',
        text: [
          'Used by privacy release modes. Auto treats emails, names, phone numbers, tax IDs, and addresses as ',
          { text: 'Direct ID', term: 'directIdentifier' },
          '; timestamps, postal codes, IDs, IPs, URLs, MACs, and country codes as ',
          { text: 'Quasi-ID', term: 'quasiIdentifier' },
          '; and other columns as Attribute. Auto does not infer ',
          { text: 'Sensitive', term: 'sensitive' },
          ', so mark private value columns yourself when ',
          { text: 'l-diversity', term: 'lDiversity' },
          ' or ',
          { text: 't-closeness', term: 'tCloseness' },
          ' should check them.',
        ],
      },
      {
        label: 'Type override',
        text: 'Use this when detection picked the wrong data type. Strategy and Role both become easier to reason about when the type is accurate.',
      },
    ],
  },
  configuration: {
    title: 'Configuration',
    summary: [
      [
        'This section decides where the output goes and which release workflow is used. Standard row-level transformation uses the per-column ',
        { text: 'Strategy', term: 'strategy' },
        ' values; formal, DP aggregate, and synthetic releases use the ',
        { text: 'Role', term: 'role' },
        ' and privacy settings.',
      ],
    ],
    points: [
      {
        label: 'Output path',
        text: 'The transformed or released CSV is written here. Use a new path unless you intentionally want to overwrite an existing file.',
      },
      {
        label: 'Local AI',
        text: [
          'Only needed for columns whose ',
          { text: 'Strategy', term: 'strategy' },
          ' is ',
          { text: 'Smart replacement', term: 'smartReplacement' },
          '. The ',
          { text: 'model', term: 'model' },
          ' runs through ',
          { text: 'Ollama', term: 'ollama' },
          ' on this device.',
        ],
      },
      {
        label: 'Privacy release',
        text: [
          'Choose Standard CSV transform for row-level transformed files, k/l/t tabular for formal row-level checks such as ',
          { text: 'k-anonymity', term: 'kAnonymity' },
          ', DP aggregate for noisy summary statistics, or Synthetic data for sampled test rows.',
        ],
      },
    ],
  },
  localAi: {
    title: 'Local AI',
    summary: [
      [
        { text: 'Local AI', term: 'localAi' },
        ' is optional. It supports ',
        { text: 'Smart replacement', term: 'smartReplacement' },
        ' by asking a downloaded ',
        { text: 'model', term: 'model' },
        ' running through ',
        { text: 'Ollama', term: 'ollama' },
        ' on this computer for realistic replacement values.',
      ],
    ],
    points: [
      {
        label: 'Data path',
        text: [
          'Rows for ',
          { text: 'Smart replacement', term: 'smartReplacement' },
          ' are sent to ',
          { text: 'localhost', term: 'localhost' },
          ', not a cloud API. Other strategies do not need ',
          { text: 'Local AI', term: 'localAi' },
          '.',
        ],
      },
      {
        label: 'Fallback',
        text: [
          'If the ',
          { text: 'model', term: 'model' },
          ' is unavailable, times out, or returns an invalid value, the app records a ',
          { text: 'fallback', term: 'fallback' },
          ' and uses rule-based pseudonymization for that value.',
        ],
      },
      {
        label: 'Review',
        text: [
          { text: 'Smart replacement', term: 'smartReplacement' },
          ' can improve readability, but it is not a formal anonymization guarantee. Use Preview and the Privacy Report to review model output and any fallbacks before relying on the file.',
        ],
      },
    ],
  },
  privacyRelease: {
    title: 'Privacy Release',
    summary: [
      [
        'Release mode controls the shape of the output. It is separate from the ',
        { text: 'Strategy', term: 'strategy' },
        ' dropdown used by standard row-level transformation.',
      ],
    ],
    points: [
      {
        label: 'Standard CSV transform',
        text: 'Writes row-level CSV data and transforms selected columns in place. It is local masking and pseudonymization, not a formal anonymity guarantee.',
      },
      {
        label: 'k/l/t tabular',
        text: [
          'Writes row-level output, redacts ',
          { text: 'Direct ID', term: 'directIdentifier' },
          ' values, generalizes ',
          { text: 'Quasi-ID', term: 'quasiIdentifier' },
          ' values, and checks ',
          { text: 'k-anonymity', term: 'kAnonymity' },
          ' plus optional ',
          { text: 'l-diversity', term: 'lDiversity' },
          ' and ',
          { text: 't-closeness', term: 'tCloseness' },
          '. ',
          { text: 'Sensitive', term: 'sensitive' },
          ' and Attribute values are kept unless you assign a different role. Suppress small classes drops rows only when that switch is enabled.',
        ],
      },
      {
        label: 'DP aggregate',
        text: 'Writes noisy count, sum, or mean results instead of source rows. Sum and mean need a numeric value column plus public lower and upper bounds. Grouped output requires public allowed group values and an Attribute-role group column. Local release history can block or warn when cumulative epsilon exceeds the configured limit. Repeatable deterministic output is not available for DP aggregate releases.',
      },
      {
        label: 'Synthetic data',
        text: [
          'Writes a complete replacement dataset from a simple per-column generator. Every CSV column is included, ',
          { text: 'Strategy', term: 'strategy' },
          ' choices such as ',
          { text: 'Smart replacement', term: 'smartReplacement' },
          ' are ignored, and ',
          { text: 'Role', term: 'role' },
          ' plus Type determine the generated placeholder shape. The same schema, row count, roles, types, and seed produce the same generated output. It does not preserve relationships between columns and does not provide a DP synthetic guarantee.',
        ],
      },
    ],
  },
  appSettings: {
    title: 'App Settings',
    summary: [
      'These settings control repeatability, output naming, DP release history, preview size, and whether paths are remembered between runs.',
    ],
    points: [
      {
        label: 'Repeatable replacements',
        text: 'Uses the configured seed so the same source values can receive the same replacements again. Treat the seed as sensitive.',
      },
      {
        label: 'Samples',
        text: 'Sample rows affect detection and preview speed. More rows can improve detection but may make loading slower on large files.',
      },
      {
        label: 'Output handling',
        text: 'Overwrite Output replaces an existing file, while Output suffix controls the default new filename.',
      },
    ],
  },
  preview: {
    title: 'Preview',
    summary: [
      'Preview shows sample before-and-after values for selected columns without writing the output file.',
    ],
    points: [
      {
        label: 'What it proves',
        text: [
          'Use it to catch obvious wrong types, wrong strategies, or ',
          { text: 'Local AI', term: 'localAi' },
          ' setup problems before running the full file.',
        ],
      },
      {
        label: 'What it does not prove',
        text: [
          'Preview is a sample, not a complete privacy review. The final run can still report ',
          { text: 'model', term: 'model' },
          ' failures, ',
          { text: 'fallbacks', term: 'fallback' },
          ', or suppressed rows.',
        ],
      },
      {
        label: 'Synthetic data',
        text: 'Synthetic data preview is disabled because the normal preview shows row-level Strategy transformations, while Synthetic data writes a generated replacement dataset.',
      },
    ],
  },
  privacyReport: {
    title: 'Privacy Report',
    summary: [
      'The report summarizes what the run actually wrote and which privacy checks passed or need review.',
    ],
    points: [
      {
        label: 'Counters',
        text: 'Counts show how many columns were treated as direct identifiers, quasi-identifiers, sensitive columns, masked columns, token columns, generalized columns, pass-through columns, and so on.',
      },
      {
        label: 'Model checks',
        text: 'Formal privacy rows show thresholds and actual values. A Review status means the output may not meet that selected privacy target.',
      },
      {
        label: 'Notes',
        text: [
          'Read notes for important caveats such as ',
          { text: 'Local AI', term: 'localAi' },
          ' ',
          { text: 'fallbacks', term: 'fallback' },
          ', DP release history, contribution bounds, or synthetic data limitations.',
        ],
      },
    ],
  },
} as const satisfies Record<string, SectionHelpEntry>

export type SectionHelpKey = keyof typeof sectionHelp
export type { HelpText, HelpTextSegment }
