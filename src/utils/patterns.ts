/**
 * Regex patterns for detecting data types in column values.
 * Used by the type detection engine to identify column content types.
 */

/**
 * Pattern for email addresses
 * Matches: user@domain.tld, user.name+tag@domain.co.uk
 */
export const EMAIL_PATTERN = /^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/;

/**
 * Pattern for UUID v4 format
 * Matches: 8-4-4-4-12 hexadecimal format (case insensitive)
 */
export const UUID_PATTERN = /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/;

/**
 * Pattern for timestamps/dates
 * Matches: YYYY-MM-DD, YYYY-MM-DDTHH:MM:SS, YYYY-MM-DD HH:MM:SS.microseconds
 */
export const TIMESTAMP_PATTERN = /^\d{4}-\d{2}-\d{2}([T ]\d{2}:\d{2}:\d{2}(\.\d+)?)?$/;

/**
 * Pattern for numeric IDs (4+ digits)
 * Matches: 1234, 12345678, etc.
 */
export const NUMERIC_ID_PATTERN = /^\d{4,}$/;

/**
 * Pattern for ISO 3166-1 alpha-2 country codes
 * Matches: US, GB, DE, etc. (exactly 2 uppercase letters)
 */
export const COUNTRY_CODE_PATTERN = /^[A-Z]{2}$/;

/**
 * Pattern for phone numbers (international format)
 * Matches: +1234567890, +1 (234) 567-8900, 1234567890, etc.
 */
export const PHONE_PATTERN = /^\+?[\d\s\-().]{10,}$/;

/**
 * Collection of all detection patterns for easy iteration
 */
export const PATTERNS = {
  email: EMAIL_PATTERN,
  uuid: UUID_PATTERN,
  timestamp: TIMESTAMP_PATTERN,
  numeric_id: NUMERIC_ID_PATTERN,
  country_code: COUNTRY_CODE_PATTERN,
  phone: PHONE_PATTERN,
} as const;
