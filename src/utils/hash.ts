/**
 * Cryptographic hashing utilities for deterministic transformations.
 * Uses Node.js crypto module for SHA-256 hashing.
 */

import { createHash } from 'crypto';

/**
 * Generate a deterministic hash from a value and seed.
 * The same input + seed combination will always produce the same output.
 *
 * @param value - The value to hash
 * @param seed - The seed for determinism (e.g., per-run secret)
 * @returns A hexadecimal hash string (64 characters)
 */
export function deterministicHash(value: string, seed: string): string {
  const combined = `${seed}:${value}`;
  return createHash('sha256').update(combined, 'utf8').digest('hex');
}

/**
 * Generate a deterministic numeric value from a hash within a range.
 * Useful for generating consistent random-looking numbers.
 *
 * @param value - The value to hash
 * @param seed - The seed for determinism
 * @param min - Minimum value (inclusive)
 * @param max - Maximum value (inclusive)
 * @returns A number between min and max
 */
export function deterministicNumber(value: string, seed: string, min: number, max: number): number {
  const hash = deterministicHash(value, seed);
  // Use first 8 characters of hash to generate a number
  const hashNum = parseInt(hash.substring(0, 8), 16);
  const range = max - min + 1;
  return min + (hashNum % range);
}

/**
 * Generate a deterministic string from a hash with specified character set and length.
 * Useful for generating consistent alphanumeric identifiers.
 *
 * @param value - The value to hash
 * @param seed - The seed for determinism
 * @param length - Desired output length
 * @param charset - Character set to use (default: lowercase alphanumeric)
 * @returns A string of the specified length
 */
export function deterministicString(
  value: string,
  seed: string,
  length: number,
  charset: string = 'abcdefghijklmnopqrstuvwxyz0123456789'
): string {
  const hash = deterministicHash(value, seed);
  let result = '';

  for (let i = 0; i < length; i++) {
    // Use 2 hex characters per output character to get better distribution
    const hexPair = hash.substring((i * 2) % 64, ((i * 2) % 64) + 2);
    const index = parseInt(hexPair, 16) % charset.length;
    result += charset[index];
  }

  return result;
}

/**
 * Generate a deterministic UUID-like string from a hash.
 * The output maintains valid UUID v4 format.
 *
 * @param value - The value to hash
 * @param seed - The seed for determinism
 * @returns A valid UUID v4 string
 */
export function deterministicUuid(value: string, seed: string): string {
  const hash = deterministicHash(value, seed);

  // Format as UUID: 8-4-4-4-12
  // Use the hash hex characters directly, but modify version (v4) and variant bits
  const parts = [
    hash.substring(0, 8),       // 8 characters
    hash.substring(8, 12),      // 4 characters
    hash.substring(12, 16),     // 4 characters (modify for version 4)
    hash.substring(16, 20),     // 4 characters (modify for variant)
    hash.substring(20, 32),     // 12 characters
  ];

  // Set version to 4 (UUID v4)
  // The 13th character should be '4'
  parts[2] = '4' + parts[2].substring(1);

  // Set variant bits (RFC 4122)
  // The 17th character should be 8, 9, a, or b
  const variantChar = parseInt(parts[3][0], 16);
  const variantBits = (variantChar & 0x3) | 0x8; // Set top 2 bits to 10
  parts[3] = variantBits.toString(16) + parts[3].substring(1);

  return parts.join('-');
}
