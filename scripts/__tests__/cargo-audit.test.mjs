import assert from 'node:assert/strict'
import test from 'node:test'
import {
  temporaryAuditExceptions,
  validateAuditExceptions,
  validateIgnoredAuditFindings,
} from '../cargo-audit.mjs'

const beforeExpiry = new Date('2026-09-30T12:00:00Z')

test('accepts the owned, rationalized, unexpired advisory declarations', () => {
  assert.doesNotThrow(() => validateAuditExceptions(temporaryAuditExceptions, beforeExpiry))
})

test('rejects malformed advisory declarations', () => {
  const malformed = [{ ...temporaryAuditExceptions[0], owner: '', rationale: '', expiresOn: '2026-02-30' }]
  assert.throws(
    () => validateAuditExceptions(malformed, beforeExpiry),
    /owner must be a non-empty string[\s\S]*rationale must be a non-empty string[\s\S]*expiresOn is not a valid calendar date/,
  )
})

test('rejects expired advisory declarations on the expiry date', () => {
  assert.throws(
    () => validateAuditExceptions(temporaryAuditExceptions, new Date('2026-10-01T00:00:00Z')),
    /exception expired on 2026-10-01/,
  )
})

test('rejects duplicate advisory declarations', () => {
  assert.throws(
    () => validateAuditExceptions([temporaryAuditExceptions[0], temporaryAuditExceptions[0]], beforeExpiry),
    /duplicate advisory exception/,
  )
})

test('rejects an additional affected version hidden by an advisory-wide ignore', () => {
  const exception = temporaryAuditExceptions[0]
  const report = {
    vulnerabilities: {
      list: [
        auditFinding(exception),
        auditFinding({ ...exception, version: '0.40.0' }),
      ],
    },
  }

  assert.throws(
    () => validateIgnoredAuditFindings([exception], report),
    /finding quick-xml@0\.40\.0 does not match declared quick-xml@0\.39\.4/,
  )
})

test('accepts findings that exactly match declared crate versions', () => {
  const report = {
    vulnerabilities: {
      list: temporaryAuditExceptions.map(auditFinding),
    },
  }

  assert.doesNotThrow(() => validateIgnoredAuditFindings(temporaryAuditExceptions, report))
})

function auditFinding(exception) {
  return {
    advisory: { id: exception.advisory },
    package: { name: exception.crate, version: exception.version },
  }
}
