# Value-First Detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make column-level PII detection value-first so names, national IDs, phones, postal codes, and addresses are detected from cell values across locales, with headers demoted to confidence boosters — per `docs/value-first-detection-design.md`.

**Architecture:** Extend the existing deterministic detector in `crates/csv-anonymizer-core/src/detection/` with new checksum validators (idsmith), a file-locale context, a name gazetteer, and per-country postal formats; then invert the pipeline in `detection.rs` so the value battery runs before header rules and checksum-validated selections cannot be suppressed by headers.

**Tech Stack:** Rust (workspace crate `csv-anonymizer-core`), `idsmith` crate (new), existing `phonenumber`, `regex`, `unicode-normalization`. No ML, no network at runtime.

## Global Constraints

- Deterministic and offline: no network calls, no randomness, no time-dependent behavior in the detection path.
- All existing tests must stay green after every task: `cargo test --workspace`.
- Lint/format gates: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` must pass before every commit.
- Dependency bar for `idsmith` (design §New detectors): permissive license (MIT/Apache-2.0/BSD), `npm run cargo:audit:required` clean, `npm run cargo:machete:required` clean. If the license is not permissive, STOP and report — do not integrate.
- Commit message convention: `lore(<scope>): <summary>` (see `git log`), scopes used here: `core`, `docs`.
- Confidence thresholds stay: High >= 80%, Medium >= 50% of non-empty samples (`scoring.rs::calculate_confidence`). New detectors must not change them.
- Header evidence may raise/lower confidence one tier and disambiguate; it may never suppress a Validator-evidence (checksum) column classification, and never create a High-risk type with zero value evidence.
- Frontend contract: no changes to `DataType` enum variants or serialized shapes (`types.rs` is mirrored in TypeScript; national IDs map to the existing `DataType::TaxId`). If a task seems to need a new `DataType`, STOP — the design maps new detections onto existing types.

---

## Phase 1: Validator expansion

### Task 1: Add idsmith behind the adoption gate

**Files:**
- Modify: `/Users/vriesd/projects/csv-data-anonymizer/Cargo.toml` (workspace `[workspace.dependencies]`)
- Modify: `crates/csv-anonymizer-core/Cargo.toml`
- Create: `crates/csv-anonymizer-core/src/detection/national_id.rs` (probe tests only in this task)
- Modify: `crates/csv-anonymizer-core/src/detection.rs` (add `mod national_id;`)

**Interfaces:**
- Consumes: `idsmith::personal_ids()`, `idsmith::tax_ids()` global registries (`.validate(country_alpha2, value)`).
- Produces (for Task 2): the verified per-country allowlist of schemes idsmith validates correctly, encoded as passing probe tests.

- [ ] **Step 1: Check the license and add the dependency**

Run: `cargo add idsmith --dry-run 2>&1 | head -5` then check the license:

```bash
cargo add idsmith -p csv-anonymizer-core
cargo metadata --format-version 1 | python3 -c "import json,sys; m=json.load(sys.stdin); print([ (p['name'],p['license']) for p in m['packages'] if p['name']=='idsmith' ])"
```

Expected: license is MIT, Apache-2.0, or BSD-family. If not: revert, STOP, report to user (adoption gate fails).

Then move the version into the workspace manifest to match repo convention: in the root `Cargo.toml` `[workspace.dependencies]` add `idsmith = "<resolved version>"`, and in `crates/csv-anonymizer-core/Cargo.toml` use `idsmith.workspace = true`.

- [ ] **Step 2: Write probe tests with known-valid/invalid national IDs**

Create `crates/csv-anonymizer-core/src/detection/national_id.rs`:

```rust
//! National-ID validation via the idsmith registries.
//! Only checksum-backed schemes are allowlisted; format-only kinds
//! (passports, driver's licenses) match arbitrary IDs too often to vote.

#[cfg(test)]
mod probe_tests {
    // Each vector is a publicly documented test identifier for the scheme.
    // A failing probe means idsmith does not validate that scheme correctly:
    // remove the country from the Task 2 allowlist instead of forcing the test.
    const VALID: &[(&str, &str, Registry)] = &[
        ("NL", "111222333", Registry::Personal),      // BSN, 11-proef
        ("BE", "85073003328", Registry::Personal),    // Rijksregisternummer, mod 97
        ("PL", "44051401359", Registry::Personal),    // PESEL
        ("IT", "RSSMRA85T10A562S", Registry::Personal), // Codice fiscale
        ("ES", "12345678Z", Registry::Personal),      // DNI, mod-23 letter
        ("FR", "255081416802538", Registry::Personal), // NIR, mod 97
        ("FI", "131052-308T", Registry::Personal),    // HETU check char
        ("SE", "811218-9876", Registry::Personal),    // Personnummer, Luhn
        ("BR", "11144477735", Registry::Personal),    // CPF
        ("DE", "86095742719", Registry::Tax),         // Steuer-IdNr (BZSt test number)
    ];

    const INVALID: &[(&str, &str, Registry)] = &[
        ("NL", "111222334", Registry::Personal),
        ("PL", "44051401358", Registry::Personal),
        ("ES", "12345678A", Registry::Personal),
        ("SE", "811218-9875", Registry::Personal),
        ("BR", "11144477736", Registry::Personal),
        ("DE", "86095742718", Registry::Tax),
    ];

    #[derive(Clone, Copy)]
    enum Registry {
        Personal,
        Tax,
    }

    fn validates(country: &str, value: &str, registry: Registry) -> bool {
        match registry {
            Registry::Personal => idsmith::personal_ids()
                .validate(country, value)
                .unwrap_or(false),
            Registry::Tax => idsmith::tax_ids().validate(country, value),
        }
    }

    #[test]
    fn idsmith_accepts_documented_valid_ids() {
        for (country, value, registry) in VALID {
            assert!(
                validates(country, value, *registry),
                "expected idsmith to accept {country} {value}"
            );
        }
    }

    #[test]
    fn idsmith_rejects_checksum_near_misses() {
        for (country, value, registry) in INVALID {
            assert!(
                !validates(country, value, *registry),
                "expected idsmith to reject {country} {value}"
            );
        }
    }
}
```

Add `mod national_id;` to the module list in `crates/csv-anonymizer-core/src/detection.rs` (after `mod header_rules;`).

API note: the `.unwrap_or(false)` / bare-bool calls above follow the idsmith 0.5 docs (`personal_ids().validate(...)` returns an `Option`/`Result`-like wrapper, `tax_ids().validate(...)` returns `bool`). If the compiler disagrees, run `cargo doc -p idsmith --no-deps` and adapt `validates()` only — the test vectors and semantics are the contract, not the call shape.

- [ ] **Step 3: Run the probes**

Run: `cargo test -p csv-anonymizer-core probe_tests -- --nocapture`

Expected: both tests PASS. If a specific country's vector fails, delete that entry from BOTH tables, note it in the commit message, and exclude that country from the Task 2 allowlist. If more than half the countries fail, the adoption gate fails: revert the dependency, STOP, and report.

- [ ] **Step 4: Run the dependency gates**

```bash
npm run cargo:audit:required
npm run cargo:machete:required
cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all --check
```

Expected: all clean. (`cargo:machete` may flag idsmith as unused until Task 2 wires it in — the `#[cfg(test)]` usage counts as used; if machete still complains, proceed to Task 2 in the same commit.)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/csv-anonymizer-core/Cargo.toml crates/csv-anonymizer-core/src/detection.rs crates/csv-anonymizer-core/src/detection/national_id.rs
git commit -m "lore(core): add idsmith behind national-id adoption-gate probes"
```

---

### Task 2: National-ID validator with test-driven allowlist

**Files:**
- Modify: `crates/csv-anonymizer-core/src/detection/national_id.rs`
- Test: same file, `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `pub(in crate::detection) fn is_national_id(value: &str) -> bool` and `pub(in crate::detection) fn national_id_countries(value: &str) -> Vec<&'static str>` (ISO alpha-2, allowlist order).
- Consumed by: Task 5 (priority battery) and Task 4 (locale-preferred attribution in the trace).

- [ ] **Step 1: Write failing tests**

Append to `national_id.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_bsn_is_national_id() {
        assert!(is_national_id("111222333"));
        assert_eq!(national_id_countries("111222333"), vec!["NL"]);
    }

    #[test]
    fn checksum_near_miss_is_not_national_id() {
        assert!(!is_national_id("111222334"));
    }

    #[test]
    fn short_or_alpha_garbage_is_skipped_cheaply() {
        assert!(!is_national_id("42"));
        assert!(!is_national_id("hello world"));
        assert!(!is_national_id(""));
    }

    #[test]
    fn spanish_dni_with_letter_validates() {
        assert!(is_national_id("12345678Z"));
        assert!(national_id_countries("12345678Z").contains(&"ES"));
    }

    #[test]
    fn german_steuer_id_validates_via_tax_registry() {
        assert!(is_national_id("86095742719"));
        assert!(national_id_countries("86095742719").contains(&"DE"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p csv-anonymizer-core national_id::tests`
Expected: FAIL — `is_national_id` not found.

- [ ] **Step 3: Implement**

Add above the test modules in `national_id.rs`:

```rust
#[derive(Clone, Copy)]
enum Scheme {
    Personal,
    Tax,
}

/// Checksum-backed schemes only, verified by the probe tests in this file.
/// Countries whose probes failed in Task 1 must not appear here.
const ALLOWLIST: &[(&str, Scheme)] = &[
    ("NL", Scheme::Personal),
    ("BE", Scheme::Personal),
    ("PL", Scheme::Personal),
    ("IT", Scheme::Personal),
    ("ES", Scheme::Personal),
    ("FR", Scheme::Personal),
    ("FI", Scheme::Personal),
    ("SE", Scheme::Personal),
    ("BR", Scheme::Personal),
    ("DE", Scheme::Tax),
];

fn scheme_validates(country: &str, value: &str, scheme: Scheme) -> bool {
    match scheme {
        Scheme::Personal => idsmith::personal_ids()
            .validate(country, value)
            .unwrap_or(false),
        Scheme::Tax => idsmith::tax_ids().validate(country, value),
    }
}

fn is_plausible_id_shape(value: &str) -> bool {
    let trimmed = value.trim();
    (6..=20).contains(&trimmed.len())
        && trimmed.chars().any(|character| character.is_ascii_digit())
        && trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '.' | ' '))
}

pub(in crate::detection) fn national_id_countries(value: &str) -> Vec<&'static str> {
    if !is_plausible_id_shape(value) {
        return Vec::new();
    }
    let trimmed = value.trim();
    ALLOWLIST
        .iter()
        .filter(|(country, scheme)| scheme_validates(country, trimmed, *scheme))
        .map(|(country, _)| *country)
        .collect()
}

pub(in crate::detection) fn is_national_id(value: &str) -> bool {
    !national_id_countries(value).is_empty()
}
```

Update the two probe-test tables to reuse `Scheme` (delete the local `Registry` enum and `validates` helper; call `scheme_validates`).

Note on US: unformatted 9-digit SSN/EIN have no checksum and stay header-gated via the existing `ssn`/`ein` crates — the US is deliberately NOT in this allowlist.

- [ ] **Step 4: Run tests**

Run: `cargo test -p csv-anonymizer-core national_id`
Expected: PASS (probes + new tests).

- [ ] **Step 5: Commit**

```bash
git add crates/csv-anonymizer-core/src/detection/national_id.rs
git commit -m "lore(core): national-id validator with checksum-backed allowlist"
```

---

### Task 3: LocaleContext type and inference

**Files:**
- Create: `crates/csv-anonymizer-core/src/detection/locale.rs`
- Modify: `crates/csv-anonymizer-core/src/detection.rs` (add `mod locale;` + re-export)

**Interfaces:**
- Produces: `pub struct LocaleContext { .. }` with `pub fn countries(&self) -> &[String]` (ISO alpha-2, most-frequent first), `Default` impl (empty), and `pub fn infer_locale_context(columns: &[Vec<String>]) -> LocaleContext`.
- Consumed by: Task 4 (threading), Task 5 (phone regions), Task 11 (postal formats).

- [ ] **Step 1: Write failing tests**

`locale.rs`:

```rust
use std::collections::HashMap;

use super::is_empty_value;
use super::validators::{is_iban, is_vat_id};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocaleContext {
    countries: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn empty_input_yields_empty_context() {
        assert!(infer_locale_context(&[]).countries().is_empty());
    }

    #[test]
    fn iban_prefixes_contribute_countries() {
        let columns = vec![column(&["NL91ABNA0417164300", "NL02RABO0123456789"])];
        assert_eq!(infer_locale_context(&columns).countries(), ["NL"]);
    }

    #[test]
    fn country_code_columns_contribute_when_dominant() {
        let columns = vec![column(&["DE", "DE", "DE", "NL", "DE"])];
        let context = infer_locale_context(&columns);
        assert_eq!(context.countries(), ["DE", "NL"]);
    }

    #[test]
    fn free_text_contributes_nothing() {
        let columns = vec![column(&["hello", "world", "US shipping soon"])];
        assert!(infer_locale_context(&columns).countries().is_empty());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p csv-anonymizer-core locale::tests`
Expected: FAIL — `infer_locale_context` / `countries` not found.

- [ ] **Step 3: Implement**

Add to `locale.rs` between the struct and the tests:

```rust
impl LocaleContext {
    pub fn countries(&self) -> &[String] {
        &self.countries
    }
}

/// ISO 3166-1 alpha-2 codes; keep in sync only if fixtures demand more.
const ISO_COUNTRY_CODES: &[&str] = &[
    "AD", "AE", "AR", "AT", "AU", "BE", "BG", "BR", "CA", "CH", "CL", "CN", "CO", "CZ", "DE",
    "DK", "EE", "EG", "ES", "FI", "FR", "GB", "GR", "HK", "HR", "HU", "ID", "IE", "IL", "IN",
    "IT", "JP", "KE", "KR", "LT", "LU", "LV", "MX", "MY", "NG", "NL", "NO", "NZ", "PE", "PH",
    "PL", "PT", "RO", "RS", "RU", "SA", "SE", "SG", "SI", "SK", "TH", "TR", "UA", "US", "VN",
    "ZA",
];

fn is_iso_country_code(value: &str) -> bool {
    ISO_COUNTRY_CODES.binary_search(&value).is_ok()
}

pub fn infer_locale_context(columns: &[Vec<String>]) -> LocaleContext {
    let mut counts: HashMap<String, usize> = HashMap::new();

    for values in columns {
        let non_empty: Vec<&String> = values
            .iter()
            .filter(|value| !is_empty_value(value))
            .collect();
        if non_empty.is_empty() {
            continue;
        }

        // Country-code column: only counts when the column is dominated by codes.
        let code_hits: Vec<&str> = non_empty
            .iter()
            .map(|value| value.trim())
            .filter(|value| value.len() == 2 && is_iso_country_code(&value.to_ascii_uppercase()))
            .collect();
        if code_hits.len() * 10 >= non_empty.len() * 8 {
            for code in &code_hits {
                *counts.entry(code.to_ascii_uppercase()).or_default() += 1;
            }
            continue;
        }

        // IBAN / VAT prefixes count per matching value.
        for value in &non_empty {
            let trimmed = value.trim();
            if is_iban(trimmed) || is_vat_id(trimmed) {
                let prefix: String = trimmed
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric())
                    .take(2)
                    .collect::<String>()
                    .to_ascii_uppercase();
                if is_iso_country_code(&prefix) {
                    *counts.entry(prefix).or_default() += 1;
                }
            }
        }
    }

    let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    LocaleContext {
        countries: ranked.into_iter().map(|(code, _)| code).collect(),
    }
}
```

In `detection.rs` add `mod locale;` and `pub use locale::{LocaleContext, infer_locale_context};`.
Note: `ISO_COUNTRY_CODES` must stay sorted for `binary_search` — add `#[test] fn iso_codes_sorted()` asserting `ISO_COUNTRY_CODES.windows(2).all(|w| w[0] < w[1])`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p csv-anonymizer-core locale`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/csv-anonymizer-core/src/detection/locale.rs crates/csv-anonymizer-core/src/detection.rs
git commit -m "lore(core): infer file-locale context from IBAN, VAT, and country columns"
```

---

### Task 4: Thread LocaleContext through detection and metadata

**Files:**
- Modify: `crates/csv-anonymizer-core/src/detection.rs`
- Modify: `crates/csv-anonymizer-core/src/detection/value.rs`
- Modify: `crates/csv-anonymizer-core/src/metadata.rs`

**Interfaces:**
- Produces: `pub fn detect_column_type_in_context(column_name: &str, values: &[String], locale: &LocaleContext) -> DetectionResult`. Existing `detect_column_type` / `detect_column_type_with_name` delegate with `&LocaleContext::default()` (signatures unchanged).
- `value.rs`: `DetectionPredicate` becomes `fn(&str, &LocaleContext) -> bool`; `detect_priority_pattern(values, total_non_empty, locale)`.
- `metadata.rs`: `build_column_metadata` computes all column vectors first, calls `infer_locale_context`, passes it down.

- [ ] **Step 1: Write the failing test (metadata-level, proves the plumbing)**

In `crates/csv-anonymizer-core/src/metadata/tests.rs` (existing test module) add:

```rust
#[test]
fn locale_context_flows_from_iban_column_to_detection() {
    // One IBAN column establishes NL context; this test only asserts the
    // plumbing compiles end-to-end and detection still classifies the IBAN
    // column. Behavioral use of the context lands in later tasks.
    let headers = vec!["iban".to_string(), "note".to_string()];
    let rows: Vec<Vec<String>> = (0..12)
        .map(|_| vec!["NL91ABNA0417164300".to_string(), "hello".to_string()])
        .collect();
    let metadata = build_column_metadata(&headers, &rows);
    assert_eq!(metadata.len(), 2);
}
```

- [ ] **Step 2: Mechanical threading**

In `detection.rs`:

```rust
pub fn detect_column_type(values: &[String]) -> crate::types::DetectionResult {
    detect_column_type_in_context("", values, &LocaleContext::default())
}

pub fn detect_column_type_with_name(
    column_name: &str,
    values: &[String],
) -> crate::types::DetectionResult {
    detect_column_type_in_context(column_name, values, &LocaleContext::default())
}

pub fn detect_column_type_in_context(
    column_name: &str,
    values: &[String],
    locale: &LocaleContext,
) -> crate::types::DetectionResult {
    // body of the old detect_column_type_with_name, with
    // detect_priority_pattern(values, total_non_empty) becoming
    // detect_priority_pattern(values, total_non_empty, locale)
}
```

In `value.rs`: change `type DetectionPredicate = fn(&str, &LocaleContext) -> bool;`, update all 13 predicate functions to accept `_locale: &LocaleContext` (ignored for now; `is_phone` uses it in Task 5), and `detect_priority_pattern` to take and forward `locale`.

In `metadata.rs`:

```rust
pub fn build_column_metadata(headers: &[String], samples: &[Vec<String>]) -> Vec<ColumnMetadata> {
    let column_values: Vec<Vec<String>> = (0..headers.len())
        .map(|index| extract_column_values(samples, index))
        .collect();
    let locale = infer_locale_context(&column_values);
    headers
        .iter()
        .enumerate()
        .map(|(index, header)| {
            build_single_column_metadata(
                header,
                index,
                &column_values[index],
                DEFAULT_SAMPLE_COUNT,
                &locale,
            )
        })
        .collect()
}
```

and `build_single_column_metadata` gains `locale: &LocaleContext`, calling `detect_column_type_in_context(name, values, locale)`.

- [ ] **Step 3: Run the full suite**

Run: `cargo test --workspace`
Expected: PASS — behavior is unchanged; this is plumbing only.

- [ ] **Step 4: Commit**

```bash
git add crates/csv-anonymizer-core/src/detection.rs crates/csv-anonymizer-core/src/detection/value.rs crates/csv-anonymizer-core/src/metadata.rs crates/csv-anonymizer-core/src/metadata/tests.rs
git commit -m "lore(core): thread locale context through detection entry points"
```

---

### Task 5: Phone regions from context + extended world list; national IDs join the battery

**Files:**
- Modify: `crates/csv-anonymizer-core/src/detection/validators.rs`
- Modify: `crates/csv-anonymizer-core/src/detection/value.rs`
- Test: `crates/csv-anonymizer-core/src/detection/tests/validators.rs` and `tests/column_type.rs`

**Interfaces:**
- `validators.rs`: `is_valid_phone_number(value)` keeps its signature (used by span detection); new `pub(super) fn is_valid_phone_number_in_context(value: &str, locale: &LocaleContext) -> bool`.
- `value.rs`: `detection_priority()` grows to 14 entries with `(DataType::TaxId, is_national_id_value)` inserted directly after the existing `(DataType::TaxId, is_tax_id)` entry.

- [ ] **Step 1: Write failing tests**

In `detection/tests/validators.rs` add:

```rust
#[test]
fn brazilian_mobile_validates_without_context() {
    // BR is not in the legacy 10-region list; the extended list must cover it.
    use crate::detection::LocaleContext;
    assert!(crate::detection::validators_test_hook_is_valid_phone_in_context(
        "(11) 91234-5678",
        &LocaleContext::default(),
    ));
}
```

In `detection/tests/column_type.rs` add:

```rust
#[test]
fn bsn_column_detects_as_tax_id_without_header() {
    let values: Vec<String> = vec!["111222333", "123456782", "111222333", "123456782"]
        .into_iter()
        .map(String::from)
        .collect();
    let result = detect_column_type_with_name("kolom3", &values);
    assert_eq!(result.data_type, DataType::TaxId);
    assert_eq!(result.confidence, Confidence::High);
}

#[test]
fn random_numeric_ids_do_not_become_tax_ids() {
    // 4+ digit sequence numbers: near-misses for every checksum scheme.
    let values: Vec<String> = vec!["100001", "100002", "100003", "100004"]
        .into_iter()
        .map(String::from)
        .collect();
    let result = detect_column_type_with_name("id", &values);
    assert_eq!(result.data_type, DataType::NumericId);
}
```

(`validators_test_hook_is_valid_phone_in_context` is a `#[cfg(test)] pub(crate)` re-export added in Step 3 — the existing test module accesses `detection` internals the same way, mirror whatever import style `tests/validators.rs` already uses.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p csv-anonymizer-core detection::tests`
Expected: the three new tests FAIL (BR phone rejected; BSN column classifies as NumericId; hook missing).

- [ ] **Step 3: Implement**

In `validators.rs`, replace `phone_region_candidates()`:

```rust
/// Broad world coverage ordered by rough likelihood; locale-context regions
/// are tried first. 200-sample cap (Task 6) bounds worst-case parse cost.
fn world_phone_regions() -> &'static [country::Id] {
    use country::Id::*;
    &[
        US, CA, GB, NL, DE, FR, ES, PT, IT, JP, BE, LU, IE, AT, CH, DK, SE, NO, FI, PL, CZ,
        SK, HU, RO, BG, GR, TR, UA, IN, CN, KR, AU, NZ, SG, HK, ID, TH, VN, PH, MY, BR, AR,
        CL, CO, MX, PE, ZA, NG, EG, KE, IL, SA, AE, RU,
    ]
}

pub(super) fn is_valid_phone_number_in_context(value: &str, locale: &LocaleContext) -> bool {
    let trimmed = value.trim();
    if trimmed.starts_with('+') {
        return parse_phone_number(None, trimmed).is_ok_and(|number| number.is_valid());
    }

    let context_regions = locale
        .countries()
        .iter()
        .filter_map(|code| code.parse::<country::Id>().ok());

    context_regions
        .chain(world_phone_regions().iter().copied())
        .any(|region| {
            parse_phone_number(Some(region), trimmed).is_ok_and(|number| number.is_valid())
        })
}

pub(super) fn is_valid_phone_number(value: &str) -> bool {
    is_valid_phone_number_in_context(value, &LocaleContext::default())
}
```

(Add `use super::locale::LocaleContext;` to validators.rs imports. Keep `is_phone` shape guards; change its body to call `is_valid_phone_number_in_context(trimmed, locale)` now that predicates receive the context from Task 4.)

In `value.rs`:

```rust
use super::national_id::is_national_id;

fn is_national_id_value(value: &str, _locale: &LocaleContext) -> bool {
    is_national_id(value)
}
```

and insert the national-id entry after `(DataType::TaxId, is_tax_id)` in `detection_priority()` (array length 13 → 14). `evidence_for(DataType::TaxId)` is already `Validator`.

Trace labels (design §Column classification pipeline promises `validator:idsmith:...` in the evidence UI): extend the priority tuple to carry a reason — `[(DataType, DetectionPredicate, &'static str); 14]` — with `"pattern rule"` for existing entries and `"validator:idsmith"` for the national-id entry, and use it instead of the hardcoded `reason: "pattern rule".to_string()` in `detect_priority_pattern`. The per-country attribution (`validator:idsmith:NL:bsn`-style detail) is appended in Task 7 when the trace is finalized: when the selected candidate's reason is `"validator:idsmith"`, look up `national_id_countries` for the first matching sample and append `":{country}"`.

Add the test hook in `detection.rs`:

```rust
#[cfg(test)]
pub(crate) fn validators_test_hook_is_valid_phone_in_context(
    value: &str,
    locale: &LocaleContext,
) -> bool {
    validators::is_valid_phone_number_in_context(value, locale)
}
```

- [ ] **Step 4: Run the full suite**

Run: `cargo test --workspace`
Expected: PASS, including all pre-existing multilingual/VAT/BTW fixtures (the false-positive guards from the June 30 benchmark).

- [ ] **Step 5: Commit**

```bash
git add crates/csv-anonymizer-core/src/detection/validators.rs crates/csv-anonymizer-core/src/detection/value.rs crates/csv-anonymizer-core/src/detection.rs crates/csv-anonymizer-core/src/detection/tests/validators.rs crates/csv-anonymizer-core/src/detection/tests/column_type.rs
git commit -m "lore(core): world phone regions and national-id battery entry"
```

---

## Phase 2: Value-first voting

### Task 6: Even-spread sampling cap

**Files:**
- Modify: `crates/csv-anonymizer-core/src/detection.rs`
- Test: `crates/csv-anonymizer-core/src/detection/tests/column_type.rs`

**Interfaces:**
- Produces: `fn sample_evenly<'a>(values: &'a [String], cap: usize) -> Vec<&'a String>` (private to `detection`); `const DETECTION_SAMPLE_CAP: usize = 200;`
- `detect_column_type_in_context` detects on the sampled subset; `DetectionResult.total_samples` still reports `values.len()`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn sampling_caps_large_columns_but_spans_the_file() {
    // 1000 rows: first 500 emails, last 500 numbers. Even sampling must see both.
    let values: Vec<String> = (0..1000)
        .map(|i| {
            if i < 500 {
                format!("user{i}@example.com")
            } else {
                format!("{i}")
            }
        })
        .collect();
    let result = detect_column_type_with_name("", &values);
    // Neither type reaches the 80% bar on an even sample; must not be High.
    assert_ne!(result.confidence, Confidence::High);
    assert_eq!(result.total_samples, 1000);
}

#[test]
fn small_columns_are_scanned_in_full() {
    let values: Vec<String> = (0..50).map(|i| format!("user{i}@example.com")).collect();
    let result = detect_column_type_with_name("", &values);
    assert_eq!(result.data_type, DataType::Email);
    assert_eq!(result.confidence, Confidence::High);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p csv-anonymizer-core sampling_caps`
Expected: first test FAILS today only if full-scan already dilutes below High — check actual behavior; if it passes trivially, strengthen it by asserting the trace's `total_non_empty <= 200` after implementation. The second test guards against regressions.

- [ ] **Step 3: Implement**

In `detection.rs`:

```rust
const DETECTION_SAMPLE_CAP: usize = 200;

fn sample_evenly(values: &[String], cap: usize) -> Vec<&String> {
    let non_empty: Vec<&String> = values
        .iter()
        .filter(|value| !is_empty_value(value))
        .collect();
    if non_empty.len() <= cap {
        return non_empty;
    }
    (0..cap)
        .map(|slot| non_empty[slot * non_empty.len() / cap])
        .collect()
}
```

`detect_column_type_in_context` builds `let sampled = sample_evenly(values, DETECTION_SAMPLE_CAP);` and passes an owned `Vec<String>` clone of the sample (or refactors downstream helpers to accept `&[&String]` — prefer the refactor; `detect_priority_pattern`, `detect_vat_value_type`, `detect_iban_value_type`, `detect_numeric_value_type` all iterate values filtering empties, so switch their parameter to the pre-filtered sample and drop their internal `is_empty_value` filters). `total_non_empty` becomes `sampled.len()`; `total_samples` stays `values.len()`.

- [ ] **Step 4: Run the full suite**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/csv-anonymizer-core/src/detection.rs crates/csv-anonymizer-core/src/detection/value.rs crates/csv-anonymizer-core/src/detection/tests/column_type.rs
git commit -m "lore(core): cap detection at 200 evenly spread samples per column"
```

---

### Task 7: Pipeline inversion — value battery first, header boosts

**Files:**
- Modify: `crates/csv-anonymizer-core/src/detection.rs` (`detect_column_type_in_context` body)
- Modify: `crates/csv-anonymizer-core/src/detection/scoring.rs` (confidence boost helper)
- Test: `crates/csv-anonymizer-core/src/detection/tests/column_type.rs`

**Interfaces:**
- Consumes: `DetectorDecision::select`, `DetectorEvidence`, existing header rules.
- Produces: new pipeline order (contract for all later tasks):
  1. value battery (VAT, IBAN, national-id, priority patterns) → `DetectorDecision`
  2. if selected evidence == `Validator` → return, with header agreement raising confidence one tier
  3. else header rules (unchanged order: early rules, numeric-id, name)
  4. else accepted value candidate, numeric, enum, string fallback
- `scoring.rs`: `pub(in crate::detection) fn raise_one_tier(confidence: Confidence) -> Confidence`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn checksum_column_beats_contradicting_header() {
    // Header says "code" (benign) but values are valid BSNs: validator wins.
    let values: Vec<String> = vec!["111222333", "123456782", "111222333", "123456782"]
        .into_iter()
        .map(String::from)
        .collect();
    let result = detect_column_type_with_name("code", &values);
    assert_eq!(result.data_type, DataType::TaxId);
}

#[test]
fn header_agreement_raises_validator_confidence_one_tier() {
    // 3 of 5 valid VAT ids -> Medium by ratio; matching header lifts to High.
    let values: Vec<String> = vec![
        "NL000099998B57",
        "NL000099998B57",
        "NL000099998B57",
        "pending",
        "pending",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let with_header = detect_column_type_with_name("btw nummer", &values);
    let without_header = detect_column_type_with_name("kolom", &values);
    assert_eq!(with_header.data_type, DataType::TaxId);
    assert_eq!(without_header.data_type, DataType::TaxId);
    assert!(with_header.confidence > without_header.confidence
        || with_header.confidence == Confidence::High);
}

#[test]
fn header_rules_still_catch_what_values_alone_cannot() {
    // 7-digit local phone format only passes the header-gated fallback shape.
    let values: Vec<String> = vec!["555-0199", "555-0142", "555-0175"]
        .into_iter()
        .map(String::from)
        .collect();
    let result = detect_column_type_with_name("telefoonnummer", &values);
    assert_eq!(result.data_type, DataType::Phone);
}
```

(Ordering note: `Confidence` derives no `Ord` — use explicit `assert_eq!(with_header.confidence, Confidence::High)` if the derive is absent; check `types.rs` first.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p csv-anonymizer-core column_type`
Expected: `checksum_column_beats_contradicting_header` FAILS today when the header rule or ordering short-circuits differently; the others may pass — keep them as guards.

- [ ] **Step 3: Implement**

`scoring.rs`:

```rust
pub(in crate::detection) fn raise_one_tier(confidence: Confidence) -> Confidence {
    match confidence {
        Confidence::Low => Confidence::Medium,
        Confidence::Medium | Confidence::High => Confidence::High,
    }
}
```

`detection.rs` — reorder `detect_column_type_in_context`:

```rust
// 1. Value battery first (was: header rules first).
let vat = detect_vat_value_type(&sampled, total_non_empty);
let iban = detect_iban_value_type(&sampled, total_non_empty);
let pattern_decision = detect_priority_pattern(&sampled, total_non_empty, locale);

// 2. Checksum-validated selections are final; header may only agree-and-boost.
//    detect_vat_value_type / detect_iban_value_type already carry Validator
//    evidence; detect_priority_pattern's selection exposes its evidence.
if let Some(result) = vat.or(iban) {
    return boost_if_header_agrees(column_name, result);
}
if let Ok(result) = &pattern_decision {
    if selected_evidence_is_validator(result) {
        return boost_if_header_agrees(column_name, result.clone());
    }
}

// 3. Header rules exactly as before (early rules, numeric-id, name) ...
// 4. ... then the non-validator pattern decision, numeric, enum, string fallback.
```

Concretely: `detect_priority_pattern` currently returns `Ok(DetectionResult)` on any accepted candidate. Split that: make it return the `DetectorDecision` (add a small wrapper `pub(in crate::detection) struct PatternOutcome { pub decision: DetectorDecision, pub trace_items: Vec<DetectionTraceItem> }`) so `detection.rs` can check `decision.selected.as_ref().map(|c| c.evidence) == Some(DetectorEvidence::Validator)` before header rules run, and otherwise defer the non-validator selection until after header rules. `boost_if_header_agrees` calls `header::terms(column_name)` + `header::best_signal_for_kinds` with the kind(s) mapping to the selected `DataType` (`tax_id` for TaxId, `phone` for Phone, `email` for Email — reuse the kind names in `header_taxonomy.json`), and on a hit applies `raise_one_tier` and appends a `DetectionTraceItem` with reason `"header agreement boost"`.

The existing early-header block, numeric-id header rule, numeric value, name header rule, enum, and string fallback keep their relative order after the validator gate.

- [ ] **Step 4: Run the full suite — this is the regression-sensitive task**

Run: `cargo test --workspace`
Expected: PASS. Failures in `multilingual_matrix` or `privacy` tests mean the inversion changed a guarded behavior — fix the ordering, not the fixture. The BTW-context, VAT-near-miss, and benign-header fixtures are the definition of "no regression" (design §False-positive control).

- [ ] **Step 5: Commit**

```bash
git add crates/csv-anonymizer-core/src/detection.rs crates/csv-anonymizer-core/src/detection/scoring.rs crates/csv-anonymizer-core/src/detection/value.rs crates/csv-anonymizer-core/src/detection/tests/column_type.rs
git commit -m "lore(core): value-first pipeline with header agreement boost"
```

---

## Phase 3: Name gazetteer

> **Withdrawn (user decision, 2026-07-06).** This entire phase — the bundled
> forename/surname datasets, the build script, the provenance doc, the
> `gazetteer.rs` runtime, and the `name_value.rs` detector — was rejected on
> data-minimization grounds: no bundled person-name datasets ship in the repo
> or its history, even public-domain ones. Name detection remains header-gated;
> header-independent name classification is future work pending a user-approved
> data source. The tasks below are retained for historical context only and
> were not shipped.

### Task 8: Gazetteer data build script + committed data + provenance

**Files:**
- Create: `scripts/build-name-gazetteer.mjs`
- Create: `crates/csv-anonymizer-core/src/detection/data/forenames.txt` (generated, committed)
- Create: `crates/csv-anonymizer-core/src/detection/data/surnames.txt` (generated, committed)
- Create: `docs/name-gazetteer-provenance.md`

**Interfaces:**
- Produces: two committed text files, one lowercase NFKD-accent-folded name per line, sorted, deduplicated, `\n` separated. Consumed by Task 9 via `include_str!`.

- [ ] **Step 1: Write the build script**

`scripts/build-name-gazetteer.mjs` downloads, normalizes, and writes the two files. Sources (all public-domain or CC0 — record exact URLs + retrieval date in the provenance doc):

1. US SSA baby names (public domain): https://www.ssa.gov/oact/babynames/names.zip — all `yobYYYY.txt` files, names with >= 25 total occurrences (drops one-off misspellings).
2. US Census Bureau 2010 surnames (public domain): https://www2.census.gov/topics/genealogy/2010surnames/names.zip — all 162k surnames.
3. sigpwned/popular-names-by-country-dataset (CC0): `common-forenames.txt` and `common-surnames.txt` from https://github.com/sigpwned/popular-names-by-country-dataset — multinational incl. CJK/RTL romanizations and native forms.

Normalization in the script: Unicode NFKD, strip combining marks (`/\p{M}/gu`), lowercase, trim; drop entries with digits, entries shorter than 2 chars, and single-letter initials; sort + dedupe. Expected output magnitude: ~90–110k forenames, ~160–180k surnames, 2–4 MB total.

The script must be deterministic given the same source files and print counts + SHA-256 of each output.

- [ ] **Step 2: Run it and commit the generated data**

Run: `node scripts/build-name-gazetteer.mjs`
Expected: both `.txt` files written with counts printed. Spot-check: `grep -x "willem" crates/csv-anonymizer-core/src/detection/data/forenames.txt` and `grep -x "jansen" .../surnames.txt` both hit; `grep -x "amsterdam" .../forenames.txt` misses.

- [ ] **Step 3: Write the provenance doc**

`docs/name-gazetteer-provenance.md`: table of source, license, URL, retrieval date, record count contributed, plus the normalization rules and the regeneration command. State explicitly that Facebook-leak-derived datasets were considered and rejected on provenance grounds.

- [ ] **Step 4: Commit**

```bash
git add scripts/build-name-gazetteer.mjs crates/csv-anonymizer-core/src/detection/data docs/name-gazetteer-provenance.md
git commit -m "lore(core): bundled name gazetteer data with provenance"
```

---

### Task 9: Gazetteer runtime module

**Files:**
- Create: `crates/csv-anonymizer-core/src/detection/gazetteer.rs`
- Modify: `crates/csv-anonymizer-core/src/detection.rs` (add `mod gazetteer;`)

**Interfaces:**
- Produces: `pub(in crate::detection) fn is_forename(token: &str) -> bool`, `pub(in crate::detection) fn is_surname(token: &str) -> bool`, `pub(in crate::detection) fn is_name_particle(token: &str) -> bool`. All accept raw tokens and normalize internally.
- Consumed by: Task 10.

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_names_across_locales_hit() {
        for name in ["Willem", "JOSÉ", "Émile", "anna"] {
            assert!(is_forename(name), "{name} should be a forename");
        }
        for name in ["Jansen", "García", "Müller", "Smith"] {
            assert!(is_surname(name), "{name} should be a surname");
        }
    }

    #[test]
    fn non_names_miss() {
        for token in ["Amsterdam42", "SKU-991", "true", ""] {
            assert!(!is_forename(token));
            assert!(!is_surname(token));
        }
    }

    #[test]
    fn dutch_and_romance_particles_recognized() {
        for particle in ["van", "der", "de", "von", "da", "di", "la"] {
            assert!(is_name_particle(particle));
        }
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p csv-anonymizer-core gazetteer`
Expected: FAIL — functions not found.

- [ ] **Step 3: Implement**

```rust
use std::collections::HashSet;
use std::sync::OnceLock;

use unicode_normalization::UnicodeNormalization;

static FORENAMES_RAW: &str = include_str!("data/forenames.txt");
static SURNAMES_RAW: &str = include_str!("data/surnames.txt");

const PARTICLES: &[&str] = &[
    "van", "vande", "vander", "den", "der", "de", "het", "ten", "ter", "von", "zu", "da",
    "das", "dos", "du", "di", "del", "della", "dela", "la", "le", "los", "mac", "mc", "bin",
    "ibn", "al", "el", "st",
];

fn forenames() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| FORENAMES_RAW.lines().filter(|line| !line.is_empty()).collect())
}

fn surnames() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| SURNAMES_RAW.lines().filter(|line| !line.is_empty()).collect())
}

fn normalize_token(token: &str) -> Option<String> {
    let trimmed = token.trim().trim_matches(|c: char| matches!(c, '.' | ',' | '\'' | '"'));
    if trimmed.len() < 2 || trimmed.chars().any(|character| character.is_ascii_digit()) {
        return None;
    }
    Some(
        trimmed
            .nfkd()
            .filter(|character| !unicode_normalization::char::is_combining_mark(*character))
            .collect::<String>()
            .to_lowercase(),
    )
}

pub(in crate::detection) fn is_forename(token: &str) -> bool {
    normalize_token(token).is_some_and(|normalized| forenames().contains(normalized.as_str()))
}

pub(in crate::detection) fn is_surname(token: &str) -> bool {
    normalize_token(token).is_some_and(|normalized| surnames().contains(normalized.as_str()))
}

pub(in crate::detection) fn is_name_particle(token: &str) -> bool {
    normalize_token(token).is_some_and(|normalized| PARTICLES.contains(&normalized.as_str()))
}
```

Add `mod gazetteer;` to `detection.rs`.

- [ ] **Step 4: Run tests + measure**

Run: `cargo test -p csv-anonymizer-core gazetteer`
Expected: PASS.
Then the binary-size check (design §Phases): `cargo build --release -p csv-anonymizer-core 2>/dev/null; ls -la target/release/ | head`. Record the `.rlib` delta in the commit message; budget is +5 MB. Over budget → switch `include_str!` to a build-script-compressed representation before proceeding (only then).

- [ ] **Step 5: Commit**

```bash
git add crates/csv-anonymizer-core/src/detection/gazetteer.rs crates/csv-anonymizer-core/src/detection.rs
git commit -m "lore(core): name gazetteer lookup with accent-folded normalization"
```

---

### Task 10: Name column detector with guards

**Files:**
- Create: `crates/csv-anonymizer-core/src/detection/name_value.rs`
- Modify: `crates/csv-anonymizer-core/src/detection.rs` (module + pipeline call)
- Test: `crates/csv-anonymizer-core/src/detection/tests/column_type.rs`

**Interfaces:**
- Produces: `pub(in crate::detection) fn detect_name_value_type(sampled: &[&String]) -> Option<(DataType, usize)>` returning the winning name type and match count, or None.
- Pipeline placement (contract with Task 7 ordering): after header rules, before the deferred non-validator pattern decision — a gazetteer classification outranks shape candidates (NumericId etc. never compete; String/Enum do) but never outranks header rules or validators.
- Thresholds (design §Name gazetteer): >= 60% column ratio; columns with < 10 non-empty samples are skipped entirely (header name rule still covers them); columns failing the uniqueness guard (`unique/sampled < 0.5`) or shaped like enums are skipped.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn dutch_forename_column_detected_without_header() {
    let values: Vec<String> = vec![
        "Willem", "Anna", "Pieter", "Sanne", "Daan", "Lotte", "Bram", "Femke", "Jeroen",
        "Nienke", "Thijs", "Maartje",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let result = detect_column_type_with_name("kolom2", &values);
    assert_eq!(result.data_type, DataType::FirstName);
}

#[test]
fn full_name_column_detected_without_header() {
    let values: Vec<String> = vec![
        "Willem Jansen", "Anna de Vries", "Pieter van den Berg", "Sanne Bakker",
        "Daan Visser", "Lotte Smit", "Bram Mulder", "Femke de Boer", "Jeroen Bos",
        "Nienke Vos", "Thijs Peters", "Maartje Hendriks",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let result = detect_column_type_with_name("kolom1", &values);
    assert_eq!(result.data_type, DataType::FullName);
}

#[test]
fn repeated_city_column_stays_enum_not_name() {
    // Cities collide with the surname set; the uniqueness guard must hold.
    let mut values = Vec::new();
    for _ in 0..6 {
        for city in ["Paris", "Berlin", "Madrid"] {
            values.push(city.to_string());
        }
    }
    let result = detect_column_type_with_name("kolom4", &values);
    assert_ne!(result.data_type, DataType::FirstName);
    assert_ne!(result.data_type, DataType::LastName);
    assert_ne!(result.data_type, DataType::FullName);
}

#[test]
fn short_columns_need_header_corroboration_for_names() {
    let values: Vec<String> = vec!["Willem", "Anna", "Pieter"]
        .into_iter()
        .map(String::from)
        .collect();
    let headerless = detect_column_type_with_name("kolom5", &values);
    assert_ne!(headerless.data_type, DataType::FirstName);
    let with_header = detect_column_type_with_name("voornaam", &values);
    assert_eq!(with_header.data_type, DataType::FirstName);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p csv-anonymizer-core column_type`
Expected: first two FAIL (String/Enum today), last two PASS today — keep as guards.

- [ ] **Step 3: Implement**

`name_value.rs`:

```rust
use std::collections::HashSet;

use crate::types::DataType;

use super::gazetteer::{is_forename, is_name_particle, is_surname};

const MIN_SAMPLES_FOR_NAME: usize = 10;
const NAME_RATIO_NUM: usize = 3; // >= 60%
const NAME_RATIO_DEN: usize = 5;

fn is_forename_cell(value: &str) -> bool {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    !tokens.is_empty() && tokens.len() <= 2 && tokens.iter().all(|token| is_forename(token))
}

fn is_surname_cell(value: &str) -> bool {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    !tokens.is_empty()
        && tokens.len() <= 4
        && tokens.iter().any(|token| is_surname(token))
        && tokens
            .iter()
            .all(|token| is_surname(token) || is_name_particle(token))
}

fn is_full_name_cell(value: &str) -> bool {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    if !(2..=6).contains(&tokens.len()) {
        return false;
    }
    let first_is_forename = is_forename(tokens[0]);
    let last_is_surname = is_surname(tokens[tokens.len() - 1]);
    let middle_ok = tokens[1..tokens.len() - 1]
        .iter()
        .all(|token| is_forename(token) || is_surname(token) || is_name_particle(token));
    // Forward "First Last" or reversed "Last First" ordering.
    middle_ok
        && ((first_is_forename && last_is_surname)
            || (is_surname(tokens[0]) && is_forename(tokens[tokens.len() - 1])))
}

pub(in crate::detection) fn detect_name_value_type(
    sampled: &[&String],
) -> Option<(DataType, usize)> {
    if sampled.len() < MIN_SAMPLES_FOR_NAME {
        return None;
    }

    let unique: HashSet<&str> = sampled.iter().map(|value| value.as_str()).collect();
    if unique.len() * 2 < sampled.len() {
        return None; // repeated values: enum-like, not a person column
    }

    let threshold = sampled.len() * NAME_RATIO_NUM / NAME_RATIO_DEN;
    let full = sampled.iter().filter(|v| is_full_name_cell(v)).count();
    let fore = sampled.iter().filter(|v| is_forename_cell(v)).count();
    let sur = sampled.iter().filter(|v| is_surname_cell(v)).count();

    // Most specific first: full names subsume forename/surname hits.
    if full > threshold {
        return Some((DataType::FullName, full));
    }
    // Forename beats surname on ties: forename lists are cleaner.
    if fore > threshold && fore >= sur {
        return Some((DataType::FirstName, fore));
    }
    if sur > threshold {
        return Some((DataType::LastName, sur));
    }
    None
}
```

In `detection.rs`, after the header name rule and before the enum check:

```rust
if let Some((name_type, match_count)) = name_value::detect_name_value_type(&sampled) {
    let confidence = scoring::calculate_confidence(match_count, total_non_empty);
    return detection_result(
        name_type,
        confidence,
        match_count,
        values.len(),
        total_non_empty,
        "Sample values matched the bundled name gazetteer.",
        vec![trace_item(
            name_type,
            "gazetteer:name",
            match_count,
            total_non_empty,
            confidence,
            true,
        )],
    );
}
```

- [ ] **Step 4: Run the full suite**

Run: `cargo test --workspace`
Expected: PASS. Watch specifically for `multilingual_matrix` regressions where columns previously classified String/Enum now classify as names — inspect each: if the fixture column IS a name column, update the fixture expectation (that's the feature); if it is NOT, tighten the guards, not the fixture.

- [ ] **Step 5: Commit**

```bash
git add crates/csv-anonymizer-core/src/detection/name_value.rs crates/csv-anonymizer-core/src/detection.rs crates/csv-anonymizer-core/src/detection/tests/column_type.rs
git commit -m "lore(core): gazetteer-backed name column detection"
```

---

## Phase 4: Address/postal tightening + fixture matrix

### Task 11: Per-country postal formats and address value voter

**Files:**
- Create: `crates/csv-anonymizer-core/src/detection/postal.rs`
- Modify: `crates/csv-anonymizer-core/src/detection/header_rules.rs` (tighten `is_postal_code` under locale)
- Modify: `crates/csv-anonymizer-core/src/detection.rs` (postal + address value voters in pipeline, after name detection, before enum)
- Test: `crates/csv-anonymizer-core/src/detection/tests/column_type.rs`

**Interfaces:**
- Produces: `pub(in crate::detection) fn postal_match_country(value: &str, locale: &LocaleContext) -> Option<&'static str>` — Some(country) when the value matches that country's postal format and the country is in the locale context (or the format is unambiguous like NL `1234 AB`).
- Address voter reuses `header_rules::is_plausible_address` (make it `pub(in crate::detection)`), with an additional street-keyword ratio gate.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn dutch_postcodes_detected_via_iban_locale_context() {
    let headers = vec!["c1".to_string(), "c2".to_string()];
    let rows: Vec<Vec<String>> = [
        ("NL91ABNA0417164300", "1012 AB"), ("NL02RABO0123456789", "2511 CV"),
        ("NL91ABNA0417164300", "3011 ED"), ("NL02RABO0123456789", "9711 LM"),
        ("NL91ABNA0417164300", "5611 EM"), ("NL02RABO0123456789", "6511 KL"),
        ("NL91ABNA0417164300", "7511 JE"), ("NL02RABO0123456789", "8011 NW"),
        ("NL91ABNA0417164300", "4811 DJ"), ("NL02RABO0123456789", "1071 XX"),
        ("NL91ABNA0417164300", "2312 EZ"), ("NL02RABO0123456789", "3512 JE"),
    ]
    .iter()
    .map(|(a, b)| vec![a.to_string(), b.to_string()])
    .collect();
    let metadata = build_column_metadata(&headers, &rows);
    assert_eq!(metadata[1].detected_type, DataType::PostalCode);
}

#[test]
fn street_address_column_detected_without_header() {
    let values: Vec<String> = vec![
        "Kerkstraat 12", "Hoofdweg 3", "Dorpsplein 8", "Molenlaan 22", "Schoolstraat 1",
        "Stationsweg 45", "Julianalaan 7", "Beatrixstraat 19", "Wilhelminaweg 30",
        "Oranjelaan 5", "Parkweg 11", "Lindenstraat 4",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let result = detect_column_type_with_name("kolom7", &values);
    assert_eq!(result.data_type, DataType::Address);
}

#[test]
fn five_digit_sku_column_is_not_postal_without_context() {
    let values: Vec<String> = (10000..10012).map(|n| n.to_string()).collect();
    let result = detect_column_type_with_name("artikel", &values);
    assert_ne!(result.data_type, DataType::PostalCode);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p csv-anonymizer-core column_type`
Expected: first two FAIL, third PASSES (guard).

- [ ] **Step 3: Implement**

`postal.rs`:

```rust
use std::sync::OnceLock;

use regex::Regex;

use super::locale::LocaleContext;

struct PostalFormat {
    country: &'static str,
    pattern: &'static str,
    /// Unambiguous shapes (letters+digits mixes) may match without locale
    /// context; bare-digit shapes (DE, FR, US...) require the country in
    /// context because they collide with SKUs and sequence numbers.
    requires_context: bool,
}

const FORMATS: &[PostalFormat] = &[
    PostalFormat { country: "NL", pattern: r"^\d{4}\s?[A-Z]{2}$", requires_context: false },
    PostalFormat { country: "GB", pattern: r"^[A-Z]{1,2}\d[A-Z\d]?\s?\d[A-Z]{2}$", requires_context: false },
    PostalFormat { country: "CA", pattern: r"^[A-Z]\d[A-Z]\s?\d[A-Z]\d$", requires_context: false },
    PostalFormat { country: "PL", pattern: r"^\d{2}-\d{3}$", requires_context: false },
    PostalFormat { country: "PT", pattern: r"^\d{4}-\d{3}$", requires_context: false },
    PostalFormat { country: "BR", pattern: r"^\d{5}-\d{3}$", requires_context: false },
    PostalFormat { country: "JP", pattern: r"^\d{3}-\d{4}$", requires_context: true },
    PostalFormat { country: "US", pattern: r"^\d{5}(?:-\d{4})?$", requires_context: true },
    PostalFormat { country: "DE", pattern: r"^\d{5}$", requires_context: true },
    PostalFormat { country: "FR", pattern: r"^\d{5}$", requires_context: true },
    PostalFormat { country: "IT", pattern: r"^\d{5}$", requires_context: true },
    PostalFormat { country: "ES", pattern: r"^\d{5}$", requires_context: true },
    PostalFormat { country: "SE", pattern: r"^\d{3}\s?\d{2}$", requires_context: true },
    PostalFormat { country: "DK", pattern: r"^\d{4}$", requires_context: true },
    PostalFormat { country: "AT", pattern: r"^\d{4}$", requires_context: true },
    PostalFormat { country: "BE", pattern: r"^\d{4}$", requires_context: true },
];

fn compiled() -> &'static Vec<(usize, Regex)> {
    static COMPILED: OnceLock<Vec<(usize, Regex)>> = OnceLock::new();
    COMPILED.get_or_init(|| {
        FORMATS
            .iter()
            .enumerate()
            .map(|(index, format)| (index, Regex::new(format.pattern).unwrap()))
            .collect()
    })
}

pub(in crate::detection) fn postal_match_country(
    value: &str,
    locale: &LocaleContext,
) -> Option<&'static str> {
    let trimmed = value.trim().to_ascii_uppercase();
    compiled().iter().find_map(|(index, regex)| {
        let format = &FORMATS[*index];
        let in_context = locale.countries().iter().any(|code| code == format.country);
        if regex.is_match(&trimmed) && (!format.requires_context || in_context) {
            Some(format.country)
        } else {
            None
        }
    })
}
```

Pipeline (in `detection.rs`, after name detection, before enum): postal voter — count samples where `postal_match_country(value, locale).is_some()`; on Medium+ ratio return `DataType::PostalCode` with trace reason `"postal:<country>"` of the most frequent country. Address voter — count `header_rules::is_plausible_address(value)` hits AND require that at least 30% of the matching values contain a street keyword (expose `header_rules::address_keywords()` as `pub(in crate::detection)`); on Medium+ ratio return `DataType::Address` with reason `"address shape + street keywords"`.

Also tighten the header-gated path: in `header_rules::detect_header_postal_code`, when `postal_match_country` hits for any sample, prefer counting those matches over the loose `is_postal_code` shape (pass `locale` through — `HeaderDetector` fn type gains a `&LocaleContext` parameter; mechanical update of the six detector fns).

- [ ] **Step 4: Run the full suite**

Run: `cargo test --workspace`
Expected: PASS, including `five_digit_sku_column_is_not_postal_without_context` and all June 30 guards.

- [ ] **Step 5: Commit**

```bash
git add crates/csv-anonymizer-core/src/detection/postal.rs crates/csv-anonymizer-core/src/detection/header_rules.rs crates/csv-anonymizer-core/src/detection.rs crates/csv-anonymizer-core/src/detection/tests/column_type.rs
git commit -m "lore(core): per-country postal formats and address value voter"
```

---

### Task 12: Locale fixture matrix — the definition of done

**Files:**
- Create: `crates/csv-anonymizer-core/src/detection/tests/locale_matrix.rs`
- Modify: `crates/csv-anonymizer-core/src/detection/tests.rs` or the tests `mod` declaration in `detection.rs` (register the module the same way `multilingual_matrix.rs` is registered)

**Interfaces:**
- Consumes: `build_column_metadata` (via `crate::metadata`) — full pipeline including locale inference.
- Produces: the per-locale triple assertion from the design's Testing section.

- [ ] **Step 1: Write the fixture harness and locale tables**

```rust
//! Per-locale triples: native headers, English headers, and no headers must
//! produce identical column classifications (design: Testing section).

use crate::metadata::build_column_metadata;
use crate::types::DataType;

struct LocaleFixture {
    locale: &'static str,
    native_headers: &'static [&'static str],
    english_headers: &'static [&'static str],
    /// Machine-style headers stand in for "no headers": col1, col2, ...
    rows: &'static [&'static [&'static str]],
    expected: &'static [DataType],
}

fn assert_triple(fixture: &LocaleFixture) {
    let generic_headers: Vec<String> = (0..fixture.expected.len())
        .map(|index| format!("col{}", index + 1))
        .collect();
    for (label, headers) in [
        ("native", to_owned(fixture.native_headers)),
        ("english", to_owned(fixture.english_headers)),
        ("headerless", generic_headers),
    ] {
        let rows: Vec<Vec<String>> = fixture
            .rows
            .iter()
            .map(|row| row.iter().map(|cell| cell.to_string()).collect())
            .collect();
        let metadata = build_column_metadata(&headers, &rows);
        for (index, expected) in fixture.expected.iter().enumerate() {
            assert_eq!(
                metadata[index].detected_type, *expected,
                "{} / {} / column {}",
                fixture.locale, label, index
            );
        }
    }
}

fn to_owned(headers: &[&str]) -> Vec<String> {
    headers.iter().map(|header| header.to_string()).collect()
}
```

Then one `#[test]` per locale. Each fixture has >= 12 rows and these columns: full name, national ID (valid checksums), phone (national format), postal code, one benign look-alike (SKU/order id) expected to stay `NumericId`/`String`. Locales: NL, DE, FR, PL, IT, ES, BR, JP, US.

Example (NL) — write the remaining eight the same way with real-format data (generate valid national IDs for fixtures with `idsmith`'s generator in a throwaway script if needed — do NOT commit invented "valid" IDs without checking them against the validator):

```rust
#[test]
fn nl_triple() {
    assert_triple(&LocaleFixture {
        locale: "NL",
        native_headers: &["volledige naam", "bsn", "telefoonnummer", "postcode", "artikelnr"],
        english_headers: &["full name", "national id", "phone", "postal code", "sku"],
        rows: &[
            &["Willem Jansen", "111222333", "+31 6 12345678", "1012 AB", "48291"],
            &["Anna de Vries", "123456782", "+31 6 23456789", "2511 CV", "48292"],
            // ... >= 12 rows total, same shape
        ],
        expected: &[
            DataType::FullName,
            DataType::TaxId,
            DataType::Phone,
            DataType::PostalCode,
            DataType::NumericId,
        ],
    });
}
```

JP caveat: the gazetteer sources romanized CJK names; if native-script Japanese names do not clear the 60% bar, the JP fixture's name column uses romanized names and a `// native-script names: known gap, tracked in design doc` comment — do not silently drop the locale.

Headerless postal caveat: bare-digit postal columns (DE/FR/US) are only detectable with locale context — each fixture must include a context-bearing column (the phone column with `+49...` prefixes does NOT contribute; include an IBAN or country-code column when the postal format `requires_context`). Adjust fixture columns accordingly (add an `iban` column to DE/FR/US/JP fixtures).

- [ ] **Step 2: Run the matrix**

Run: `cargo test -p csv-anonymizer-core locale_matrix -- --nocapture`
Expected: all locale triples PASS. Failures here are the point of the whole project — debug the detector, not the fixture, unless the fixture data itself is wrong (invalid checksum, wrong phone format).

- [ ] **Step 3: Full verification sweep**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
npm run cargo:audit:required && npm run cargo:machete:required
```

Expected: all clean.

- [ ] **Step 4: Update the design doc status and the old phased plan**

In `docs/value-first-detection-design.md` change `Status:` to `Implemented (see docs/value-first-detection-plan-2026-07-05.md)`. In `docs/multilingual-detection-phased-plan.md`, add a pointer note at the top that value-first detection (this plan) supersedes the "future improvements" items it overlaps.

- [ ] **Step 5: Commit**

```bash
git add crates/csv-anonymizer-core/src/detection/tests/locale_matrix.rs crates/csv-anonymizer-core/src/detection.rs docs/value-first-detection-design.md docs/multilingual-detection-phased-plan.md
git commit -m "lore(core): per-locale fixture matrix proves header-independent detection"
```
