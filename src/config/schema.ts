import { z } from 'zod';

/**
 * Schema for valid data types that can be anonymized
 */
export const DataTypeSchema = z.enum([
  'email',
  'uuid',
  'timestamp',
  'numeric_id',
  'country_code',
  'phone',
  'first_name',
  'last_name',
  'full_name',
  'enum',
  'string',
  'unknown',
]);

/**
 * Schema for strategy-specific options
 */
export const StrategyOptionsSchema = z
  .object({
    deterministic: z.boolean().optional(),
    preserveDomain: z.boolean().optional(),
    preserveDigitCount: z.boolean().optional(),
    preservePrecision: z.boolean().optional(),
    offsetDays: z.number().int().positive().max(3650).optional(),
  })
  .strict()
  .optional();

/**
 * Schema for a single column's configuration
 */
export const ColumnConfigSchema = z.object({
  name: z
    .string()
    .min(1, 'Column name cannot be empty')
    .describe('Column name must match the CSV header'),
  type: DataTypeSchema.optional().describe('Override auto-detected type'),
  strategy: z.string().optional().describe('Custom strategy name'),
  options: StrategyOptionsSchema.describe('Strategy-specific options'),
});

/**
 * Schema for the complete anonymization configuration file
 */
export const ConfigSchema = z.object({
  columns: z
    .array(ColumnConfigSchema)
    .min(1, 'At least one column must be configured')
    .describe('List of columns to anonymize'),
  output: z
    .string()
    .optional()
    .describe('Output file path (default: {input}_anonymized.csv)'),
  deterministic: z
    .boolean()
    .optional()
    .default(false)
    .describe('Use deterministic transformations globally'),
  seed: z
    .string()
    .optional()
    .describe('Seed for deterministic mode (required if deterministic is true)'),
});

/**
 * Inferred types from schemas for runtime validation
 */
export type DataTypeSchemaType = z.infer<typeof DataTypeSchema>;
export type StrategyOptionsSchemaType = z.infer<typeof StrategyOptionsSchema>;
export type ColumnConfigSchemaType = z.infer<typeof ColumnConfigSchema>;
export type ConfigSchemaType = z.infer<typeof ConfigSchema>;

/**
 * Validate a configuration object against the schema.
 * Returns the validated config or throws ConfigValidationError.
 */
export function validateConfig(config: unknown): ConfigSchemaType {
  return ConfigSchema.parse(config);
}

/**
 * Safely validate a configuration object.
 * Returns a result object with success status and either data or error.
 */
export function safeValidateConfig(config: unknown): z.SafeParseReturnType<unknown, ConfigSchemaType> {
  return ConfigSchema.safeParse(config);
}
