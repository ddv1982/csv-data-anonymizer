import type { ParseError } from 'papaparse';

export function isNonFatalParseWarning(error: ParseError): boolean {
  return error.type === 'Delimiter' && error.code === 'UndetectableDelimiter';
}

export function getFatalParseError(errors: ParseError[]): ParseError | undefined {
  return errors.find(error => !isNonFatalParseWarning(error));
}
