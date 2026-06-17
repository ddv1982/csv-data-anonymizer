/**
 * Generic string anonymization strategy.
 * Fallback strategy for generic string columns.
 * Generates random lorem ipsum or identifier strings while preserving approximate length.
 */

import { faker } from '@faker-js/faker';
import type { Strategy } from './index.js';
import type { TransformContext } from '../types/index.js';
import { isEmptyValue } from './index.js';
import { deterministicString, deterministicNumber } from '../utils/hash.js';

/**
 * Generate a random string of approximately the target length.
 * @param targetLength - Desired approximate length
 * @param deterministic - Whether to use deterministic generation
 * @param value - Original value for deterministic hashing
 * @param seed - Seed for deterministic hashing
 * @returns A random string with length within ±20% of target
 */
function generateString(
  targetLength: number,
  deterministic: boolean,
  value: string,
  seed: string
): string {
  // Calculate acceptable length range (±20%)
  const minLength = Math.max(1, Math.floor(targetLength * 0.8));
  const maxLength = Math.ceil(targetLength * 1.2);

  if (deterministic) {
    // Determine output length deterministically
    const outputLength = deterministicNumber(value, seed + ':length', minLength, maxLength);

    // Generate deterministic string
    return deterministicString(
      value,
      seed + ':content',
      outputLength,
      'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-'
    );
  }

  // Random generation using Faker.js
  // Choose between different generation strategies based on length
  if (targetLength <= 10) {
    // Short strings: use word or identifier
    const word = faker.lorem.word({ length: { min: minLength, max: maxLength } });
    return word.substring(0, maxLength);
  }

  if (targetLength <= 50) {
    // Medium strings: use sentence
    const words = faker.lorem.words(Math.ceil(targetLength / 6));
    // Trim or pad to approximately target length
    if (words.length > maxLength) {
      return words.substring(0, maxLength);
    }
    if (words.length < minLength) {
      // Pad with additional words
      const additional = faker.lorem.words(2);
      return (words + ' ' + additional).substring(0, maxLength);
    }
    return words;
  }

  // Long strings: use paragraph
  const paragraph = faker.lorem.paragraph(Math.ceil(targetLength / 50));
  if (paragraph.length > maxLength) {
    return paragraph.substring(0, maxLength);
  }
  if (paragraph.length < minLength) {
    // Pad with additional paragraphs
    const additional = faker.lorem.paragraph();
    return (paragraph + ' ' + additional).substring(0, maxLength);
  }
  return paragraph;
}

/**
 * Generic string anonymization strategy implementation.
 * Generates random strings while preserving approximate length (±20%).
 */
export const genericStringStrategy: Strategy = {
  transform(value: string, context: TransformContext): string {
    // Preserve empty values
    if (isEmptyValue(value)) {
      return value;
    }

    // Get original length
    const targetLength = value.length;

    // Handle very short strings
    if (targetLength === 0) {
      return value;
    }

    // Generate replacement string
    return generateString(targetLength, context.deterministic, value, context.seed);
  },
};
