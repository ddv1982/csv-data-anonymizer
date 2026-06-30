# Multilingual Detection Phased Plan

Date: 2026-06-30

## Purpose

This plan turns the multilingual header detection investigation into an
implementation roadmap. It focuses on improving PII detection for CSV headers and
sample values in multiple languages while preserving the app's local-first privacy
model and explainable detector behavior.

Related research: `docs/multilingual-header-detection-investigation.md`.

## Implementation Status

Status as of 2026-06-30:

- Phase 0 baseline measurement exists as table-driven Rust detector fixtures for
  English, Dutch, German, French, Spanish, Portuguese, Italian, and a Japanese
  pilot.
- Phase 1 Unicode-safe header normalization is implemented with
  `unicode-normalization`, `unicode-segmentation`, compact keys, accent folding,
  and camelCase splitting.
- Phase 2/3 deterministic header taxonomy and scored header evidence are
  implemented through `crates/csv-anonymizer-core/src/detection/header_taxonomy.json`.
- Phase 4 value validation is implemented for the current bounded validator set:
  `phonenumber` for phone values, `iban_validate` for IBAN values,
  `vat_id_validator` for prefixed VAT IDs, `email_address` for strict email
  parsing, `url` for HTTP(S) URL parsing, and `card-validate` for payment-card
  brand/length/Luhn checks. Bare Dutch `BTW` / `omzetbelastingnummer` values are
  detected only under Dutch BTW header context.
- Phase 4a reference fixtures are implemented with checked-in VAT cases and the
  optional `scripts/verify_stdnum_vat_fixtures.py` dev check against
  `python-stdnum`.
- Phase 4b conservative fuzzy header matching is implemented for longer taxonomy
  terms with sample-value confirmation. Private/user event date headers remain
  exact-only to avoid substring false positives such as `candidateOfBirth`.
- Phase 5 evidence exposure is implemented through privacy evidence summaries,
  detector source labels, and multi-detector source lists.
- Phase 6/7 semantic embeddings, GLiNER/NER, cloud DLP, and Local AI classifier
  assistance remain future opt-in experiments. They are not part of the default
  detector path.

## Product Boundary

This plan improves automatic data-type and privacy-risk detection. It does not
localize the app UI. After the first taxonomy release, "supported languages" should
mean:

- deterministic header taxonomy coverage exists for that language,
- regression fixtures prove expected detection behavior,
- detector evidence labels explain which rule or term matched,
- and value detectors still run independently of header language.

The default detector must remain local, deterministic where possible, and
reviewable. Cloud PII APIs, hidden LLM calls, and heavy native model runtimes are
out of scope for the default path.

## Guiding Decisions

1. Prefer value evidence over header wording.
   Exact validators and strong value-shape rules should outrank header-only
   signals.
2. Make headers Unicode-safe before adding languages.
   A taxonomy cannot work well if normalization strips non-ASCII text first.
3. Move from boolean inference to scored evidence.
   The app should be able to explain "what matched" and "how strong it was".
4. Treat multilingual terms as maintained detector data.
   Terms should be testable, reviewed, and versioned instead of scattered through
   Rust conditionals.
5. Keep semantic and Local AI classifiers optional until measured.
   They can assist ambiguous columns, but should not silently override exact
   validators.

## Phase 0: Baseline And Measurement

Goal: establish a measurable baseline before changing detector behavior.

Scope:

- Add multilingual fixture CSV schemas for the current high-value PII concepts:
  email, phone, first name, last name, full name, address, postal code, date of
  birth, account number, tax or government ID, URL, IP address, and device ID.
- Include equivalent headers in English, Dutch, German, French, Spanish,
  Portuguese, Italian, and one non-Latin script fixture.
- Add negative fixtures for ordinary business columns that should not become PII:
  status, product code, category, quantity, invoice total, region, department, and
  free-form notes.
- Capture current detection output as explicit failing or pending expectations
  where multilingual behavior is not implemented yet.

Deliverables:

- A small detector evaluation module or test helper in
  `crates/csv-anonymizer-core`.
- Regression tests documenting current English behavior.
- A table of expected multilingual detections in the test code or a fixture file.

Acceptance criteria:

- Current English header detections still pass.
- New multilingual tests prove the gap without changing production behavior yet.
- Test names clearly distinguish "baseline gap" from "required behavior".

Risks:

- If tests assert too much before implementation, they will create noise. Use
  pending fixtures or helper comments until each language pack lands.

Recommended PR size: 1 small PR.

## Phase 1: Unicode-Safe Header Normalization

Goal: make header parsing preserve and normalize multilingual text without adding
large dependencies.

Scope:

- Add lightweight Unicode dependencies such as `unicode-normalization` and
  `unicode-segmentation`.
- Replace ASCII-only normalization in
  `crates/csv-anonymizer-core/src/detection/header.rs`.
- Generate multiple matching keys per header:
  raw normalized text, lowercase text, diacritic-insensitive Latin text, token
  list, and compact form.
- Preserve non-Latin tokens instead of dropping them.
- Keep camelCase splitting for existing English headers.
- Keep existing `infer_*` behavior compatible while the new normalized structure
  is introduced.

Implementation notes:

- Prefer NFKC or NFC for canonical normalization.
- For Latin-script matching, also produce an accent-folded key so `telefono` and
  `teléfono`, or `prenom` and `prénom`, can share taxonomy entries.
- Do not globally strip marks from non-Latin scripts.
- Continue to preserve compact matching for headers without whitespace.

Deliverables:

- A new `HeaderTerms` representation with Unicode-aware tokens and compact keys.
- Unit tests for accented Latin headers, mixed punctuation, camelCase,
  snake_case, kebab-case, dotted names, and non-Latin headers.
- Compatibility tests for existing English terms such as `firstName`,
  `date_of_birth`, `api_token`, `phoneNumber`, and `postal_code`.

Acceptance criteria:

- No existing detector tests regress.
- `HeaderTerms` no longer drops non-ASCII header content.
- Existing English detections behave the same as before.

Risks:

- Unicode case folding and tokenization can change edge cases. Keep the first PR
  focused on normalization, not new semantic matches.

Recommended PR size: 1 small to medium PR.

## Phase 2: Scored Header Signals

Goal: replace direct boolean header inference with evidence that can be fused,
tested, and displayed.

Scope:

- Introduce a detector signal model for header evidence.
- Convert existing header detectors from boolean `infer_*` checks into scored
  signals.
- Preserve the public `DetectionResult` behavior initially.
- Include detector source, data type, confidence, score, reason, matched term or
  concept, and sample-shape guard result.
- Keep value evidence as the stronger signal when there is a conflict.

Suggested internal shape:

```rust
struct DetectorSignal {
    source: DetectorSource,
    data_type: DataType,
    confidence: Confidence,
    score: u8,
    reason: String,
    matched: String,
    match_count: usize,
    total_considered: usize,
}

enum DetectorSource {
    ValuePattern,
    ValueValidator,
    HeaderTaxonomy,
    HeaderLegacyRule,
    LocalAiClassifier,
    UserOverride,
}
```

Deliverables:

- Header inference emits one or more `DetectorSignal`s.
- Existing detection trace generation uses signal reasons instead of duplicated
  strings where practical.
- Tests cover competing signals, for example an ambiguous `id` header with
  non-integer values should not force `NumericId`.

Acceptance criteria:

- Existing result shape exposed to the frontend remains stable.
- Detection reasons become more specific, not less specific.
- The code no longer needs to hard-code every confidence decision inside each
  individual `detect_header_*` function.

Risks:

- This is a structural change. Keep it behavior-preserving before adding the
  multilingual taxonomy.

Recommended PR size: 1 medium PR.

## Phase 3: Data-Driven Multilingual Taxonomy V1

Goal: move header concepts out of Rust conditionals and ship deterministic
coverage for the first supported language pack.

Scope:

- Add a versioned taxonomy data file under `crates/csv-anonymizer-core`, for
  example `data/header_taxonomy.toml` or `data/header_taxonomy.json`.
- Load it with `include_str!` and parse at startup or with `OnceLock`.
- Encode each entry with concept ID, data type, language, term, weight,
  normalized matching mode, and optional required sample guard.
- Start with these languages:
  English, Dutch, German, French, Spanish, Portuguese, Italian.
- Include a small non-Latin pilot set for the most unambiguous concepts, such as
  email, phone, address, and name, to verify the tokenizer path.

Example data shape:

```toml
[[term]]
concept = "person.first_name"
data_type = "firstName"
lang = "nl"
text = "voornaam"
weight = 92
requires_sample_guard = "plausible_name_part"

[[term]]
concept = "contact.phone"
data_type = "phone"
lang = "es"
text = "telefono"
weight = 94
requires_sample_guard = "phone_like"
```

Deliverables:

- Taxonomy loader and validator tests.
- First reviewed language pack.
- Migration of current English literal terms into taxonomy entries.
- Multilingual detection tests for:
  `telefono`, `teléfono`, `telefoon`, `telefon`, `adresse`, `adres`,
  `direccion`, `geboortedatum`, `geburtsdatum`, `fecha_nacimiento`,
  `voornaam`, `achternaam`, `prenom`, `apellido`, `postcode`, `codigo_postal`,
  `plz`, `rekeningnummer`, and `kontonummer`.

Acceptance criteria:

- Current English behavior remains compatible.
- Target language headers detect correctly when sample values support the
  taxonomy signal.
- Header-only broad terms produce at most medium confidence unless the concept is
  highly specific.
- Ambiguous terms such as `nr`, `number`, `code`, `naam`, and `id` require value
  shape or stronger context before auto-selecting sensitive handling.

Risks:

- Translation quality directly affects false positives. Keep entries reviewed,
  source comments in the data file where useful, and tests for both positive and
  negative examples.

Recommended PR size: 1 medium to large PR, or split by loader first and language
pack second.

## Phase 4: International Value Validators

Goal: reduce dependence on header language by strengthening sample-value
evidence.

Scope:

- Evaluate a libphonenumber-compatible path for international phone detection.
- Add or improve IBAN detection as a high-confidence account identifier signal.
- Add VAT and national identifier validators only where formats are distinctive
  enough to avoid high false positives. Prefixed VAT IDs should use
  `vat_id_validator`; local Dutch `BTW` / `omzetbelastingnummer` values without
  an `NL` prefix should remain header-context gated.
- Improve postal-code and address evidence without assuming English street
  suffixes.
- Make region-specific validators explicit in detector labels, for example
  `validator:tax-id:us` or `validator:iban`.

Deliverables:

- Dependency decision notes for each validator added or rejected.
- New validator tests with international examples and false-positive cases.
- Updated detection traces that distinguish generic shape rules from
  jurisdiction-specific validators.
- Optional `python-stdnum` fixture verification for VAT examples without adding a
  Python runtime dependency to the app.

Acceptance criteria:

- International phone examples detect without relying on English headers.
- IBAN-shaped values detect as account identifiers even when the header is not
  English.
- Prefixed VAT values with valid checksums detect as tax IDs; invalid checksum
  near misses do not.
- Dutch bare BTW tax numbers detect only under Dutch BTW header context.
- Address detection improves for common multilingual examples but remains
  conservative.
- False-positive rate on negative fixtures does not increase beyond the agreed
  threshold from Phase 0.

Risks:

- Some validators require country context. Avoid pretending that every national ID
  can be detected globally with high confidence.
- Heavy native dependencies, especially address parsers, may be inappropriate for
  the desktop bundle. Prefer bounded validators first.

Recommended PR size: multiple small PRs by validator.

## Phase 5: Evidence UI And User Overrides

Goal: make multilingual detector decisions inspectable and correctable.

Scope:

- Surface detector evidence in the existing column review UI where practical.
- Show source labels such as `Header taxonomy`, `Value validator`, `Pattern`, or
  `Local AI classifier`.
- Keep manual type override as the authoritative user decision.
- Consider adding a project-level custom taxonomy file only after built-in
  taxonomy behavior is stable.

Deliverables:

- Frontend rendering for detector evidence already present in metadata, or a
  backend metadata extension if evidence is not currently exposed in enough
  detail.
- Tests covering displayed evidence for at least one taxonomy match and one value
  validator match.
- Documentation explaining that detection is best-effort and reviewable.

Acceptance criteria:

- A user can tell why a multilingual header was selected.
- A user can override a wrong detection before output creation.
- Evidence labels do not imply cloud processing or hidden AI when the default
  deterministic path was used.

Risks:

- Too much evidence can clutter the workflow. Show concise labels by default and
  leave detailed traces behind disclosure where needed.

Recommended PR size: 1 frontend/backend integration PR after Phases 2-4.

## Phase 6: Semantic Header Classifier Proof Of Concept

Goal: determine whether local embeddings reduce taxonomy maintenance without
hurting precision.

Scope:

- Prototype outside the default detector path.
- Compare taxonomy-only detection against embedding-assisted detection on the
  Phase 0 fixture corpus.
- Evaluate latency, model size, packaging complexity, threshold stability, and
  false positives for short ambiguous headers.
- Treat embedding output as a `DetectorSignal`, never as an unconditional
  override.

Deliverables:

- Prototype branch or feature-flagged module.
- Evaluation report with precision, recall, false positives, latency, and bundle
  impact.
- Explicit ship/no-ship recommendation.

Acceptance criteria:

- Embeddings improve recall on multilingual headers that are not already covered
  by taxonomy.
- False positives stay within the threshold set in Phase 0.
- The feature can be disabled completely.

Risks:

- Short column names are semantically weak.
- Model assets and runtime dependencies can make packaging and support harder
  than the detection gain justifies.

Recommended PR size: prototype only. Do not merge into the default path until the
evaluation is convincing.

## Phase 7: Optional Local AI Detector Assist

Goal: reuse the existing Ollama path for ambiguous columns after deterministic
evidence has been exhausted.

Scope:

- Only run when Local AI is enabled and ready.
- Only run for ambiguous columns, not every column.
- Send header plus bounded sample summaries to localhost Ollama.
- Require strict JSON output and validation.
- Display `Local AI classifier` as evidence.
- Never let Local AI override exact validators or explicit user choices.

Deliverables:

- Prompt and schema for classification output.
- Backend validator for model responses.
- UI evidence label and user setting.
- Tests with mocked Local AI responses for valid, invalid, low-confidence, and
  conflicting suggestions.

Acceptance criteria:

- Local AI assist is opt-in.
- All processing stays on localhost.
- Invalid model responses are ignored safely.
- The user sees when a suggestion came from Local AI.

Risks:

- Different local models may behave differently. Keep confidence conservative and
  require deterministic evidence or user confirmation for sensitive auto-actions.

Recommended PR size: 1 feature-flagged experimental PR after Phase 5.

## Suggested Release Sequence

1. Release A: Phase 0 and Phase 1.
   This is infrastructure only: baseline fixtures plus Unicode-safe header
   handling.
2. Release B: Phase 2 and Phase 3.
   This is the first visible multilingual detection improvement.
3. Release C: Phase 4.
   This improves detection even when headers are weak or mixed-language.
4. Release D: Phase 5.
   This makes the new evidence easier to inspect and override.
5. Release E: Phase 6 or Phase 7 only if measurement justifies it.

## Tracking Metrics

Track these metrics for each phase:

- recall for high-risk and medium-risk PII auto-selection,
- precision by data type,
- false positives on non-PII business columns,
- ambiguous-column deferral rate,
- detection latency per 100 columns,
- taxonomy size and load time,
- and number of user-facing manual overrides in test scenarios.

Initial target:

- Preserve existing English precision.
- Improve recall for target multilingual headers without increasing false
  positives on negative fixtures.
- Keep default analysis latency effectively unchanged for normal CSV samples.

## Completed Implementation Slice

The first deterministic implementation slice includes:

1. Add multilingual detector fixtures and current-behavior tests.
2. Add Unicode normalization and tokenization helpers.
3. Preserve all existing English detections.
4. Add normalization tests proving accented and non-Latin headers are retained.
5. Move header concepts into the JSON taxonomy.
6. Add stronger phone and IBAN validators.
7. Keep VAT/tax detection conservative with false-positive regression tests.

This creates a stable foundation for later optional semantic detector work and
gives clear evidence that future phases improve real detection coverage instead
of merely moving literal words around.
