/**
 * Email anonymization strategy.
 * Anonymizes the local part of email addresses while preserving the domain.
 */

import { faker } from '@faker-js/faker';
import type { Strategy } from './index.js';
import type { TransformContext } from '../types/index.js';
import { isEmptyValue } from './index.js';
import { deterministicString } from '../utils/hash.js';

/**
 * Extract the domain part from an email address.
 * @param email - The email address
 * @returns The domain part (including @) or empty string if invalid
 */
function extractDomain(email: string): string {
  const atIndex = email.lastIndexOf('@');
  if (atIndex === -1) {
    return '';
  }
  return email.substring(atIndex);
}

/**
 * Generate a fake local part for an email address.
 * @param deterministic - Whether to use deterministic generation
 * @param value - Original value for deterministic hashing
 * @param seed - Seed for deterministic hashing
 * @returns A fake email local part
 */
function generateLocalPart(deterministic: boolean, value: string, seed: string): string {
  if (deterministic) {
    // Generate a deterministic username using hash
    // Format: word + number (e.g., "user427" or "fake983")
    const prefix = deterministicString(value, seed + ':prefix', 6, 'abcdefghijklmnopqrstuvwxyz');
    const suffix = deterministicString(value, seed + ':suffix', 3, '0123456789');
    return `${prefix}${suffix}`;
  }

  // Use Faker.js for random generation
  const firstName = faker.person.firstName().toLowerCase();
  const lastName = faker.person.lastName().toLowerCase();
  const num = faker.number.int({ min: 1, max: 999 });

  // Randomly choose a format
  const formats = [
    `${firstName}.${lastName}`,
    `${firstName}${lastName}`,
    `${firstName}${num}`,
    `${firstName}.${lastName}${num}`,
  ];

  const format = formats[faker.number.int({ min: 0, max: formats.length - 1 })];
  return format.replace(/[^a-z0-9.]/g, ''); // Clean up any special characters
}

/**
 * Email anonymization strategy implementation.
 * Preserves the domain part while anonymizing the local part.
 */
export const emailStrategy: Strategy = {
  transform(value: string, context: TransformContext): string {
    // Preserve empty values
    if (isEmptyValue(value)) {
      return value;
    }

    // Extract domain from original email
    const domain = extractDomain(value);
    if (!domain) {
      // If no valid domain found, return original value
      // (shouldn't happen if type detection is correct)
      return value;
    }

    // Generate new local part
    const localPart = generateLocalPart(context.deterministic, value, context.seed);

    return `${localPart}${domain}`;
  },
};
