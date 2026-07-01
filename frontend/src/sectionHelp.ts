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
    title: 'Review Sensitive Columns',
    summary: [
      'Choose which detected columns should change. Unchecked columns stay unchanged.',
    ],
    points: [
      {
        label: 'Defaults',
        text: [
          'CSV File and Paste Sample preselect medium/high-risk columns and set them to ',
          { text: 'Redact', term: 'redact' },
          '.',
        ],
      },
      {
        label: 'Review signals',
        text: [
          { text: 'Risk', term: 'risk' },
          ' and evidence explain why a column was flagged. If the type looks wrong, choose ',
          { text: 'Redact', term: 'redact' },
          ', ',
          { text: 'Mask', term: 'mask' },
          ', or ',
          { text: 'Tokenize', term: 'tokenize' },
          '.',
        ],
      },
      {
        label: 'Methods',
        text: [
          'Auto and ',
          { text: 'Pseudonymize', term: 'pseudonymize' },
          ' create readable fake values. ',
          { text: 'Redact', term: 'redact' },
          ', ',
          { text: 'Mask', term: 'mask' },
          ', and ',
          { text: 'Tokenize', term: 'tokenize' },
          ' are stricter. ',
          { text: 'Smart replacement', term: 'smartReplacement' },
          ' uses ',
          { text: 'Local AI', term: 'localAi' },
          '; ',
          { text: 'Pass through', term: 'passThrough' },
          ' keeps originals.',
        ],
      },
      {
        label: 'Run behavior',
        text: [
          'Show Preview samples selected columns without writing a file. Create protected CSV writes the output. Repeated source values reuse replacements within the current run.',
        ],
      },
    ],
  },
  configuration: {
    title: 'Configuration',
    summary: [
      [
        'This section decides where the protected CSV goes. The file is transformed with the per-column ',
        { text: 'Strategy', term: 'strategy' },
        ' values from the column review.',
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
  appSettings: {
    title: 'App Settings',
    summary: [
      'These settings control output naming, preview size, and whether paths are remembered between runs.',
    ],
    points: [
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
          'Preview is a sample, not a complete privacy review. Rule-based preview replacements are examples; the final run generates its own replacements while keeping repeated source values consistent inside that run. Smart replacement preview values are reused when available. The final run can still report ',
          { text: 'model', term: 'model' },
          ' failures, ',
          { text: 'fallbacks', term: 'fallback' },
          ', or values that used rule-based replacement.',
        ],
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
        text: 'Counts show how many columns were treated as direct identifiers, quasi-identifiers, masked columns, token columns, pass-through columns, and so on.',
      },
      {
        label: 'Notes',
        text: [
          'Read notes for important caveats such as ',
          { text: 'Local AI', term: 'localAi' },
          ' ',
          { text: 'fallbacks', term: 'fallback' },
          ' or columns left unchanged.',
        ],
      },
    ],
  },
} as const satisfies Record<string, SectionHelpEntry>

export type SectionHelpKey = keyof typeof sectionHelp
export type { HelpText, HelpTextSegment }
