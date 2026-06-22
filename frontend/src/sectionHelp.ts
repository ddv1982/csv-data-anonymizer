export const sectionHelp = {
  selectFile: {
    title: 'Select File',
    summary: [
      'Start by choosing the CSV you want to anonymize. The app reads the header row and a sample of rows so it can detect column types, risk, and suggested masking settings before you write any output.',
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
    title: 'Select Columns',
    summary: [
      'Choose which columns need protection, then adjust the detected type, Strategy, and Role where the automatic guess is not enough.',
    ],
    points: [
      {
        label: 'Strategy',
        text: 'Used by Standard masking. Auto and Pseudonymize run type-based replacements; some types such as booleans, country codes, percentages, currency, and enums can stay unchanged. Mask replaces every non-space character with *, Tokenize writes stable tok_ values, Smart replacement uses Local AI, and Pass through keeps the original value.',
      },
      {
        label: 'Role',
        text: 'Used by privacy release modes. Auto treats emails, names, phone numbers, tax IDs, and addresses as Direct ID; timestamps, postal codes, IDs, IPs, URLs, MACs, and country codes as Quasi-ID; and other columns as Attribute. Auto does not infer Sensitive, so mark sensitive columns yourself when l-diversity or t-closeness should check them.',
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
      'This section decides where the output goes and which release workflow is used. Standard masking uses the per-column Strategy values; formal, DP aggregate, and synthetic releases use the Role and privacy settings.',
    ],
    points: [
      {
        label: 'Output path',
        text: 'The anonymized or released CSV is written here. Use a new path unless you intentionally want to overwrite an existing file.',
      },
      {
        label: 'Local AI',
        text: 'Only needed for columns whose Strategy is Smart replacement. The model runs through Ollama on this device.',
      },
      {
        label: 'Privacy release',
        text: 'Choose Standard masking for row-level transformed files, k/l/t tabular for formal row-level checks, DP aggregate for noisy summary statistics, or Synthetic data for generated example-like rows.',
      },
    ],
  },
  localAi: {
    title: 'Local AI',
    summary: [
      'Local AI is optional. It supports Smart replacement by asking a model running through Ollama on this computer for realistic replacement values.',
    ],
    points: [
      {
        label: 'Data path',
        text: 'Rows for Smart replacement are sent to localhost, not a cloud API. Other strategies do not need Local AI.',
      },
      {
        label: 'Fallback',
        text: 'If the model is unavailable or returns an invalid value, the app records a fallback and uses rule-based pseudonymization for that value.',
      },
      {
        label: 'Review',
        text: 'Smart replacements can be useful for readability, but they are not a formal anonymization guarantee. Use Preview and the Privacy Report to review the result.',
      },
    ],
  },
  privacyRelease: {
    title: 'Privacy Release',
    summary: [
      'Release mode controls the shape of the output. It is separate from the Strategy dropdown used by Standard masking.',
    ],
    points: [
      {
        label: 'Standard masking',
        text: 'Writes row-level CSV data and transforms selected columns in place. It is local masking and pseudonymization, not a formal anonymity guarantee.',
      },
      {
        label: 'k/l/t tabular',
        text: 'Writes row-level output, redacts Direct ID values, generalizes Quasi-ID values, and checks k-anonymity plus optional l-diversity and t-closeness. Sensitive and Attribute values are kept unless you assign a different role. Suppress small classes drops rows only when that switch is enabled.',
      },
      {
        label: 'DP aggregate',
        text: 'Writes noisy count, sum, or mean results instead of source rows. Sum and mean need a numeric value column plus public lower and upper bounds. Repeated releases spend additional privacy budget and must be tracked outside the file.',
      },
      {
        label: 'Synthetic data',
        text: 'Writes generated rows from simple per-column distributions. It does not preserve relationships between columns, and a synthetic DP epsilon records intent only because this MVP does not implement a DP synthesizer.',
      },
    ],
  },
  appSettings: {
    title: 'App Settings',
    summary: [
      'These settings control repeatability, output naming, preview size, and whether paths are remembered between runs.',
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
        text: 'Use it to catch obvious wrong types, wrong strategies, or Local AI setup problems before running the full file.',
      },
      {
        label: 'What it does not prove',
        text: 'Preview is a sample, not a complete privacy review. The final run can still report model failures, fallbacks, or suppressed rows.',
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
        text: 'Read notes for important caveats such as Local AI fallbacks, deterministic seed sensitivity, DP budget accounting, or synthetic data limitations.',
      },
    ],
  },
} as const

export type SectionHelpKey = keyof typeof sectionHelp
