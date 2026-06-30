# Multilingual Header Detection Investigation

Date: 2026-06-30

Status: historical investigation plus implementation notes. The original
"Current Code Findings" section describes the detector before the multilingual
implementation. The current implementation now uses Unicode-aware header
normalization, a data-driven taxonomy, conservative fuzzy matching for longer
non-date taxonomy terms, detector evidence labels, `phonenumber` for phone
validation, `iban_validate` for IBAN validation, `vat_id_validator` for prefixed
VAT IDs, parser-backed email and HTTP(S) URL checks, and `card-validate` for
payment-card validation. Remaining future work is primarily broader fixture
coverage, optional semantic/ML experiments, and more jurisdiction-specific
tax/reference validation.

## Question

The current detector uses English header terms such as `firstName`, `phone`, `address`,
`ssn`, and `dateOfBirth`. This investigation looks at how to support multilingual
CSV headers and multilingual personal data detection without continuing to embed
literal English word lists directly in Rust code.

## Executive Summary

There is no reliable drop-in library that can infer every multilingual CSV column
meaning from headers alone. Header semantics require one or more of these:

- a maintained concept taxonomy or dictionary,
- value-shape validators,
- named-entity recognition over sample values,
- multilingual semantic embeddings,
- or an LLM-style classifier.

The best fit for this app is a layered detector:

1. Keep deterministic value detectors as the highest-confidence layer.
2. Make header parsing Unicode-aware, not ASCII-only.
3. Move header concepts into a data-driven multilingual taxonomy.
4. Add stronger international validators for phone numbers, addresses, and country
   or jurisdiction-specific identifiers.
5. Optionally add a local semantic header classifier, using embeddings or the
   existing Ollama/Local AI path, but keep it opt-in or clearly evidenced.

This preserves the app's local-first privacy model and avoids silently sending CSV
schema or values to cloud services.

## Original Code Findings

This section records the pre-implementation state that motivated the change.
It is kept for design context, not as a description of the current code.

The current code already has two different detector classes:

- Value-pattern detectors in `crates/csv-anonymizer-core/src/detection.rs`.
- Header-word detectors in `crates/csv-anonymizer-core/src/detection/header.rs`.

The value-pattern layer is mostly language-agnostic for email, UUID, IP address,
MAC address, URL, numeric IDs, and payment-card or IBAN-shaped values. However,
some value rules are region-specific, such as US SSN/EIN tax ID shape, `$`
currency, ISO-like timestamps, and US-style inline phone numbers.

The header layer is where multilingual support currently breaks:

- `header.rs` contains hard-coded English terms for secrets, account numbers,
  numeric IDs, private dates, phones, postal codes, addresses, tax IDs, and names.
- `compact()` and `tokens()` use `is_ascii_alphanumeric()`, so non-Latin headers
  can be stripped to little or nothing before matching.
- Name detection is especially header-dependent. The value-side plausibility check
  accepts Unicode alphabetic characters, but it only runs after English header
  inference says the column is a name.
- Address detection is also English-biased because the value plausibility check
  looks for suffixes such as `street`, `road`, `avenue`, and `lane`.
- `metadata.rs` uses detected type and privacy evidence to choose risk and default
  strategy, so missed multilingual header signals can lead to missed auto-selection.

This means a Spanish `telefono`, Dutch `achternaam`, German `geburtsdatum`, French
`adresse`, or Japanese `電話番号` header can be treated as a low-risk string even
when the equivalent English header would be flagged.

## Evaluated Options

### 1. Unicode-Aware Header Normalization

The immediate baseline fix is to stop treating headers as ASCII-only. Use Unicode
normalization and Unicode word segmentation before concept matching.

Useful Rust crates:

- `unicode-normalization` for Unicode normalization forms.
- `unicode-segmentation` for Unicode text segmentation.

This does not solve semantics by itself, but it prevents data loss for accented and
non-Latin headers and makes the next layers possible.

Recommended behavior:

- Normalize headers with NFKC or NFC.
- Case-fold/lowercase where applicable.
- Split on punctuation, whitespace, underscores, dashes, dots, and camelCase.
- Keep non-Latin tokens instead of discarding them.
- Preserve a compact form for languages/scripts where no whitespace appears.

### 2. Data-Driven Multilingual Concept Taxonomy

Move header concepts out of Rust code and into versioned data files, for example:

```toml
[[concept]]
id = "person.full_name"
data_type = "fullName"
privacy_kind = "person"
base_risk = "high"
labels = [
  { lang = "en", text = "full name" },
  { lang = "nl", text = "volledige naam" },
  { lang = "de", text = "vollstaendiger name" },
  { lang = "fr", text = "nom complet" },
  { lang = "es", text = "nombre completo" }
]
requires_value_shape = "plausible_person_name"
```

This still uses words, but it changes the problem from scattered hard-coded checks
to a testable taxonomy with weights, evidence, language packs, and release notes.

This is the lowest-risk improvement path. It is transparent, deterministic, small
enough to ship, easy to review, and easy for users to override later.

### 3. Stronger International Value Validators

Header language matters less when value validators are strong. The app should
continue shifting confidence toward value evidence wherever possible.

Useful libraries or references:

- Google's libphonenumber, or a Rust crate backed by equivalent metadata, for
  international phone parsing and validation.
- libpostal for international address parsing/normalization.
- python-stdnum as a reference set for country-specific personal, tax, VAT, bank,
  and business identifiers. This is Python, not Rust, so the app should use it as
  a dev/test oracle rather than a runtime dependency.
- Existing deterministic validators for IBAN, payment cards, UUIDs, IPs, URLs,
  and MAC addresses should remain first-class.

Tradeoffs:

- libphonenumber-style validation is a strong fit and relatively bounded.
- libpostal is powerful but heavy, with native build and data-size implications
  that may be uncomfortable for a lightweight desktop app.
- Government/tax identifiers are jurisdiction-specific. A single global "tax ID"
  detector will either miss data or false-positive heavily unless it has country
  context or country-specific validators.
- Prefixed VAT IDs can be validated locally with `vat_id_validator`. Bare Dutch
  `BTW` / `omzetbelastingnummer` values should be treated separately because they
  are not VIES-style prefixed VAT IDs and should remain Dutch-header gated.

### 4. Presidio-Style Recognizer Architecture

Microsoft Presidio is the best design reference even if it is not an immediate
runtime dependency. It uses recognizers, language-aware NLP engines, allow/deny
lists, regexes, context words, and custom recognizer registries.

For this Rust app, the relevant idea is not "embed Presidio"; it is:

- define independent recognizers,
- have each recognizer emit evidence and confidence,
- allow language-specific recognizer packs,
- make recognizer configuration data-driven,
- and explain the winning evidence in the UI.

A Python Presidio sidecar would bring packaging and runtime complexity. It may be
appropriate for a future optional "advanced detector engine", but not for the
default desktop path.

### 5. Multilingual Header Embeddings

Semantic embeddings are the most promising way to reduce dependence on literal
header wording while staying local.

Approach:

1. Define canonical PII concepts with labels and descriptions.
2. Precompute embeddings for those concepts.
3. Embed the column header, and optionally nearby sample-value summaries.
4. Compare cosine similarity against concept embeddings.
5. Emit this as evidence, not as an unconditional override.

Candidate model families:

- Sentence Transformers multilingual MiniLM models.
- E5 multilingual embedding models.

Why this fits:

- Header strings are small, so latency can be manageable.
- It avoids enumerating every synonym in every language.
- It can run locally via ONNX Runtime or another local inference path.
- It keeps CSV data on-device.

Risks:

- Threshold calibration is mandatory.
- Short headers such as `id`, `nr`, `num`, `code`, or `naam` are ambiguous.
- Embeddings can produce plausible but wrong semantic matches.
- Bundled model size and native inference dependencies affect packaging.

This should be introduced behind a feature flag or settings toggle until measured.

### 6. Local AI Header Classifier

The existing Ollama integration can also classify headers and sample shapes. A
prompt could ask for JSON such as:

```json
{
  "column": "telefono",
  "suggestedDataType": "phone",
  "privacyKind": "contact",
  "confidence": "medium",
  "reason": "Header appears to mean phone number and sample values match phone-like strings."
}
```

This is attractive because the app already has Local AI settings, model download,
status checks, and JSON validation infrastructure.

However, Local AI should not become the hidden default detector:

- It is slower than deterministic rules.
- It can be inconsistent across models.
- It may produce false confidence unless constrained carefully.
- It depends on a user-installed model/runtime.

Recommended role: optional "Detector assist" for columns where deterministic
evidence is weak or ambiguous. The UI should label it as Local AI evidence.

### 7. Cloud PII APIs

Google Sensitive Data Protection, AWS Comprehend PII, and Azure AI Language PII
all provide broad managed PII detection. They are useful benchmarks, but they do
not fit this app's default privacy boundary because data would leave the device.

They should not be used by default. If ever supported, they should be explicit,
opt-in connectors with strong UI warnings and no silent fallback.

## Recommended Architecture

Introduce a `DetectorSignal` model and fuse evidence from multiple recognizers.

```rust
struct DetectorSignal {
    source: DetectorSource,
    data_type: DataType,
    privacy_kind: Option<PrivacyFindingKind>,
    confidence: Confidence,
    score: u8,
    reason: String,
    match_count: usize,
    total_considered: usize,
}

enum DetectorSource {
    ValuePattern,
    ValueValidator,
    HeaderTaxonomy,
    HeaderEmbedding,
    ValueNer,
    LocalAiClassifier,
    UserOverride,
}
```

Pipeline:

1. Parse values and run exact validators.
2. Run broad value-shape detectors.
3. Normalize/tokenize headers with Unicode support.
4. Score headers against the multilingual concept taxonomy.
5. Optionally score headers with local embeddings.
6. Optionally run local NER or Local AI on sample values for text-heavy columns.
7. Fuse evidence with conservative thresholds.
8. Preserve all candidate evidence for UI explanations and tests.

Confidence rules should prefer value evidence:

- Exact value validator: can be high confidence.
- Header taxonomy plus compatible sample shape: medium or high depending on type.
- Header taxonomy alone: usually low or medium, never silent high for broad terms.
- Embedding-only signal: medium at most until calibrated.
- Local AI signal: evidence source only; do not let it override exact validators.
- User override: explicit high-confidence selection.

## Suggested Milestones

### Milestone 1: Refactor Without New Heavy Dependencies

- Replace ASCII-only header normalization with Unicode-aware normalization.
- Move English header terms into a data file.
- Replace boolean `infer_*` functions with scored `HeaderSignal`s.
- Add tests for headers in at least Dutch, German, French, Spanish, Portuguese,
  and one non-Latin script.
- Keep current behavior as compatibility tests.

### Milestone 2: Improve International Value Validators

- Improve phone detection with international number parsing.
- Add more robust postal/address shape evidence without assuming English street
  suffixes.
- Add country-aware or format-aware validators for IBAN, VAT, and common national
  IDs where the format is distinctive. Keep `python-stdnum` as a fixture oracle
  and keep runtime detection in Rust.
- Keep region-specific detectors explicit in evidence labels.

### Milestone 3: Multilingual Taxonomy Pack

- Add a reviewed first language pack for likely user data: English, Dutch,
  German, French, Spanish, Portuguese, and Italian.
- Store each term with concept, language, weight, and required value-shape guard.
- Add a user-extensible custom taxonomy file later if demand appears.

### Milestone 4: Semantic Header Classifier Proof of Concept

- Prototype a local embedding model on a multilingual fixture corpus.
- Compare taxonomy-only versus embedding-assisted precision/recall.
- Decide whether to ship model assets, download them on demand, or keep this as
  an optional advanced feature.

### Milestone 5: Optional Local AI Detector Assist

- Reuse the Ollama pathway for ambiguous columns only.
- Require structured JSON output and strict validation.
- Show "Local AI classifier" as evidence.
- Never send data outside localhost.
- Keep manual user confirmation before output creation.

## Evaluation Harness

Create fixtures with equivalent schemas in multiple languages:

- `email`, `correo`, `e-mail`, `courriel`
- `phone`, `telefono`, `telefon`, `telephone`, `telefoon`
- `first_name`, `voornaam`, `prenom`, `nombre`, `vorname`
- `last_name`, `achternaam`, `nom`, `apellido`, `nachname`
- `date_of_birth`, `geboortedatum`, `fecha_nacimiento`, `geburtsdatum`
- `address`, `adresse`, `direccion`, `adres`
- `postal_code`, `postcode`, `codigo_postal`, `plz`
- `account_number`, `rekeningnummer`, `kontonummer`, `numero_compte`

Track:

- recall for high/medium PII auto-selection,
- false-positive rate on ordinary business columns,
- per-data-type precision and recall,
- latency per 100 columns,
- model or taxonomy size,
- and explainability quality in the UI.

## Recommendation

Do not replace the current detector with a single library or LLM call. The safest
path is to evolve it into a recognizer pipeline:

1. Unicode-aware header handling.
2. Data-driven multilingual header taxonomy.
3. Stronger international value validators.
4. Optional local semantic assist for ambiguous headers.
5. Evidence-based UI and regression fixtures.

This gives practical multilingual support quickly while preserving the app's
local-first posture and keeping detector decisions inspectable.

## Primary Sources Reviewed

- Microsoft Presidio analyzer language support and recognizer design:
  https://microsoft.github.io/presidio/analyzer/languages/
- Microsoft Presidio recognizer registry provider:
  https://microsoft.github.io/presidio/analyzer/recognizer_registry_provider/
- spaCy trained pipeline and language model catalog:
  https://spacy.io/models
- Google libphonenumber:
  https://github.com/google/libphonenumber
- libpostal:
  https://github.com/openvenues/libpostal
- python-stdnum:
  https://arthurdejong.org/python-stdnum/
- Lingua language detector for Rust:
  https://github.com/pemistahl/lingua-rs
- Unicode normalization crate:
  https://docs.rs/unicode-normalization/latest/unicode_normalization/
- Unicode segmentation crate:
  https://docs.rs/unicode-segmentation/latest/unicode_segmentation/
- Sentence Transformers multilingual MiniLM model card:
  https://huggingface.co/sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2
- Multilingual E5 model card:
  https://huggingface.co/intfloat/multilingual-e5-small
- GLiNER:
  https://github.com/urchade/GLiNER
- Google Sensitive Data Protection infoTypes:
  https://cloud.google.com/sensitive-data-protection/docs/infotypes-reference
