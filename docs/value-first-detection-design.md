# Value-First Detection Design

Date: 2026-07-05
Status: Approved design, pending implementation plan

## Problem

Column-level PII detection is header-dependent for the categories that carry the
most privacy weight. Names, addresses, dates of birth, postal codes, and bare
national-ID formats are only detected when the header matches the 241-term,
8-language taxonomy. Real files break this in four ways, all confirmed against
user experience:

1. Name and address columns are missed when headers are cryptic, abbreviated,
   or in an unsupported language.
2. Files from locales outside the current coverage (8 taxonomy languages, 10
   hardcoded phone regions, US/EU-only ID validators) degrade silently.
3. National IDs such as BSN, Steuer-IdNr, PESEL, and codice fiscale fall
   through to generic `NumericId` (medium risk) instead of `GovernmentId`.
4. Headerless files and machine-generated headers (`col_1`, database dumps)
   get no header evidence at all.

The root cause is architectural: header evidence gates detection for these
types instead of merely boosting it.

## Design Principle

Classify every column primarily from its **values**. Headers boost confidence
and disambiguate; they never gate. A column whose sampled values are 90% valid
BSNs is a `GovernmentId` column regardless of what the header says, and a
headerless file must classify identically to the same file with headers.

Everything stays deterministic, offline, and traceable — no ML, no cloud, no
model downloads in the default path. This preserves the conclusions of
`detector-library-evaluation.md`: the in-house detector architecture remains;
we extend its validator battery and re-weight its evidence, we do not adopt a
third-party detector engine.

## Architecture

### Column classification pipeline (revised)

1. **Sample** up to 200 non-empty values per column, spread evenly across the
   file (today all rows are scanned; sampling caps cost as the validator
   battery grows — full scan remains for files under the cap).
2. **Run all value detectors** on each sample: existing validators (email,
   phonenumber, iban, card, VAT, SSN/EIN), new idsmith validators, gazetteer
   matchers (names), and shape/pattern detectors. Each hit yields
   `(DataType, evidence tier, optional country)`.
3. **Vote per column**: match ratio against existing thresholds (High >= 80%,
   Medium >= 50%). Ties resolve by evidence tier (validator > gazetteer >
   pattern > shape), then by the existing specificity ranking.
4. **Header adjustment**: a taxonomy match may raise or lower confidence one
   tier and select among ambiguous value-level candidates (e.g. `order date`
   vs `birth date` for a date column). It cannot create a High-risk
   classification with zero value evidence, and it cannot suppress a
   checksum-validated value classification.
5. **File-locale context**: countries observed in checksum-validated columns
   (IBAN prefixes, VAT prefixes, country-code columns) form a candidate-locale
   set used to disambiguate country-ambiguous formats and to seed phone-region
   parsing.

The existing `DetectionTrace` / evidence UI carries through unchanged; new
detectors report as `validator:idsmith:<country>:<kind>` and
`gazetteer:forename` / `gazetteer:surname` so decisions stay reviewable.

### New detectors

**idsmith (checksum validators).** Adds personal IDs (97 formats), tax IDs
(80), bank accounts (159), passports, driver's licenses, and broader IBAN/VAT
coverage — all format- plus checksum-validated, cross-validated upstream
against python-stdnum (already used by `stdnum:vat:check` fixtures). Per
value, validation is attempted against the candidate-locale set first, then
all countries. A column votes on the specific `(country, id kind)` pair so the
UI can say "Dutch BSN" rather than "some ID". Adoption gate: must win fixtures
(section: Testing) and pass the dependency bar (license, build impact,
`cargo audit`/`machete` clean) before it replaces or extends any existing
validator path.

**Name gazetteer.** A bundled dictionary of ~100–300k forenames and surnames
from clean-provenance sources (Wikipedia-derived popular-names-by-country,
government open-data name lists), compiled at build time into a compact
static structure (FST or perfect-hash; target a few MB in the binary).
Matching is case- and accent-normalized, multi-script. Column rules:

- >= 60% of samples in forename set → `FirstName`
- >= 60% in surname set → `LastName`
- >= 60% match `<forename token> <surname-or-capitalized token>` (or the
  reverse order) → `FullName`

Single-cell gazetteer hits never flag a free-text column as Person; the
threshold is column-level. Span-level (inline) detection is unchanged.

**Phone regions.** Replace the hardcoded 10-region list: parse `+`-prefixed
numbers directly, then try candidate locales from file context, then fall back
to all regions. Column-level voting keeps the cost bounded (once a region
validates consistently, it is tried first for remaining samples).

**Addresses and postal codes.** Keep the conservative heuristics but run them
as value voters instead of header-gated rules: "digit + capitalized tokens"
shapes plus the street-word list for addresses; per-country postal formats
(from the candidate-locale set) tighten the currently loose 3–12-char shape.

### False-positive control

- Checksum validators stay hard-gated: invalid checksum = no match.
- Country-ambiguous formats (a 9-digit string valid as both SSN shape and
  BSN) resolve by: checksum validity first, then candidate-locale context,
  then header hint, then the more conservative (lower-risk) classification.
- Gazetteer thresholds are column-level ratios; short columns (< 10 non-empty
  values) require header corroboration for Person classification, mirroring
  the existing enum-detection guard.
- The existing conservative behaviors that won the June 30 benchmark (BTW
  header context, VAT near-miss rejection, benign-header handling) are
  preserved as fixtures and must keep passing.

## What does not change

- Two-stage pipeline (column type + cell spans), `ColumnMetadata`,
  `PrivacyFindingKind`, risk mapping, and strategy selection.
- The header taxonomy itself — it remains valuable as booster/disambiguator
  and for types with no value signal (e.g. `username` columns).
- Anonymization strategies and the frontend evidence/review flow (new detector
  labels are additive).
- Local AI / embeddings remain future opt-in phases per
  `multilingual-detection-phased-plan.md` Phases 6–7, now scoped to the
  residual ambiguity left after value-first classification.

## Testing

Extend the fixture matrix with per-locale CSV triples — native headers,
English headers, **no headers** — for at least NL, DE, FR, PL, IT, ES, BR, JP,
US. Each triple must produce identical column classifications; that assertion
is the definition of done for the language-independence goal. Fixtures
include: person-name columns (native scripts), national IDs (valid and
checksum-near-miss), phones in national formats, addresses, postal codes, and
benign look-alikes (SKUs, order IDs, sequence numbers) that must stay
unflagged. The June 30 benchmark fixtures are folded in as regression guards.

## Phases

1. **Validator expansion** — idsmith integration behind the adoption gate,
   phone-region rework, file-locale context plumbing. Pure detector additions;
   no scoring changes.
2. **Value-first voting** — sampling, per-column voting, header re-weighting
   (boost/disambiguate, never gate). The architectural core.
3. **Name gazetteer** — data sourcing/licensing, build-time compilation,
   column rules, binary-size check.
4. **Address/postal tightening + fixture matrix completion** — per-country
   postal formats, address voter, full per-locale triple matrix.

Each phase lands with its fixtures and must keep the false-positive guards
green. Phase order is chosen so the riskiest architectural change (Phase 2)
ships after the validator battery it depends on exists.
