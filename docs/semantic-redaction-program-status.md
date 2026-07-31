# Semantic redaction reliability program

This document tracks the broader reliability program around semantic detection and
redaction. It deliberately distinguishes implemented behavior from planned work.
The current contract is described in
[`robust-semantic-redaction-implementation.md`](robust-semantic-redaction-implementation.md).

## Non-negotiable baseline

Future phases must preserve these properties:

- UUID shape alone means a persistent/record identifier, not a network/device ID.
- Generic UUID identifiers remain Medium risk and protected by default.
- Device-specific context is required to refine a UUID into a network/device ID.
- Redact uses one constant marker for an uncertain column and retains no
  distinct-value mapping.
- Numbered, equality-preserving markers belong only to the explicitly selected
  Label strategy.
- A typed marker such as `[PERSON]` requires supporting semantic evidence.

## Delivery status

| Area | Current state | Remaining work |
| --- | --- | --- |
| Safe UUID semantics | Implemented | Extend and calibrate contextual terms without weakening the generic fallback. |
| Constant column-derived Redact markers | Implemented | Keep preview, reports, and every input format on the same decision path. |
| Explicit numbered Label strategy | Implemented | Keep linkability and memory consequences visible to users. |
| Structured evidence profile | Implemented | Continue extending the policy without collapsing format and meaning. |
| Backend-authoritative marker preview | Implemented | Keep all new callers on the serialized backend decision. |
| Deterministic semantic policy matrix | Implemented for current marker kinds | Extend the matrix whenever a new typed marker is introduced. |
| Multilingual contextual taxonomy | Partial | Add language packs with positive, negative, and conflicting fixtures. |
| Calibration corpus and quality budgets | Implemented baseline | Grow the locked synthetic corpus and add independent external evaluation data when available. |
| Property and fuzz testing | Property baseline implemented | Add a scheduled mutation-fuzzing job when CI provides a stable time budget. |
| Cross-format equivalence | Implemented for structured inputs | Extend equivalence reporting to intentional free-text span differences. |
| Performance and memory gates | Partial | Record baselines and enforce tolerances in a stable CI environment. |
| Compatibility and controlled rollout | Not complete | Version the metadata contract, run shadow comparisons, and test rollback before deleting legacy fields. |

“Partial” means useful tests or infrastructure exist, not that the phase has met
its exit gate.

## Phased completion plan

### 1. Authoritative decision contract

Return the backend's exact marker and its provenance with every analyzed column.
Strategy and type overrides must cause the backend to recompute that decision.
Keep legacy fields during a compatibility window.

Exit gate: preview, final output, and privacy report consume the same backend
decision, with no frontend implementation of marker selection.

### 2. First-class evidence and policy

Represent these independently:

- structural format, confidence, match count, and sample count;
- semantic kind, confidence, specificity, and resolved/uncertain/conflicting state;
- privacy risk, selection recommendation, and explanation;
- output marker, marker source, typed/generic status, and equality preservation.

Typed markers must be issued only by an explicit policy rule whose supporting
evidence is available for review.

Exit gate: detection, risk, selection, and transformation consume one structured
profile, and data-driven tests name the rule behind every typed marker.

### 3. Taxonomy and calibration corpus

Grow contextual terms by concept and language. Every term needs an exact match
mode, compatible formats, weight, negative examples, and a declaration of whether
it can justify a typed marker. Short ambiguous tokens must use exact matching.

Maintain separate development, adversarial, multilingual, cross-format, and
locked held-out partitions. Use generated or licensed fixtures; never put customer
examples into product rules.

Exit gate: every taxonomy change has positive, negative, conflicting, and relevant
multilingual coverage, and the held-out partition is not used for tuning.

### 4. Quality measurement and invariant testing

Report format and semantic precision/recall separately, plus sensitive-column
recall, false auto-selection, typed-marker precision, generic-marker rate,
incorrect-specific-marker rate, and review-required rate.

Zero-tolerance gates:

- protected source values are never passed through;
- preview and final output never disagree;
- equivalent structured inputs never disagree without a documented exception.

Add property and fuzz coverage for arbitrary headers and values. Redact must be
deterministic, bounded, free of source fragments and control characters, and
independent of source cardinality.

Exit gate: CI publishes a readable quality report and fails on an agreed budget.

### 5. Interface, formats, and performance

The review interface should show format, meaning, uncertainty/conflict, risk, and
the exact backend marker. Label must clearly disclose that it preserves repeated
value relationships. Verify keyboard and screen-reader behavior for every state.

Run equivalent decisions through CSV, pasted CSV, JSON, YAML, XML, and supported
free-text paths. Document intentional field-versus-span differences.

Performance coverage currently includes:

- streaming CSV transforms at fixed row count;
- unique versus repeated values for Redact, Label, and Pseudonymize;
- the general detector matrix;
- UUID detection under generic, device-specific, contradictory, and multilingual
  header contexts;
- direct-input paste paths.

Run with:

```text
cargo bench -p csv-anonymizer-core --bench csv_streaming
cargo bench -p csv-anonymizer-core --bench detector_matrix
cargo bench -p csv-anonymizer-core --bench direct_input_paste
```

Criterion detects time regressions against a local saved baseline. It does not
measure peak resident memory and should not be treated as a CI memory gate.
Before enforcing timing budgets, record baselines on pinned hardware with fixed
Rust and dependency versions. Add a separate process-level RSS harness for
high-cardinality Redact and Label workloads. Redact should remain flat with
cardinality; Label is expected to grow with distinct values.

Exit gate: supported formats agree, UX states pass accessibility tests, and pinned
runtime/RSS budgets show no unacceptable regression.

### 6. Compatibility and controlled rollout

Prepared-analysis schema version 3 introduces the authoritative evidence profile.
Version-2 snapshots are rejected explicitly with a re-analysis error instead of
falling through to an integrity mismatch. CLI, desktop IPC, reports, and frontend
contracts move together.

Roll out the structured profile in stages:

1. compute legacy and new decisions in shadow mode;
2. record only aggregate decision categories, never raw values;
3. review marker, risk, and selection differences;
4. canary-enable the new policy;
5. make it the default with a tested rollback;
6. remove legacy fields only after the compatibility window.

Risk decreases and removal of automatic selection require manual review.

Exit gate: no unresolved high-severity differences remain, quality and performance
budgets pass, migrations work, and rollback has been exercised.

## Benchmark interpretation

The detector benchmark is a regression signal, not an accuracy score. The UUID
context family holds values constant and varies headers, isolating taxonomy and
evidence-aggregation cost. The streaming cardinality family holds row count and
value width constant while varying distinct count. Its Redact pair exercises the
constant marker path; its Label and Pseudonymize pairs expose mapping costs.

Benchmark results must be stored with CPU, operating system, Rust version, build
profile, fixture size, and commit. Do not compare Criterion numbers gathered on
different machines as a release gate.

## Known gaps

The repository now has a single backend authority, a serialized evidence profile,
structured cross-format checks, property-style invariants, and a synthetic calibration
quality gate. Operational rollout gates remain environment-dependent: Criterion
benchmarks are not enforced timing/RSS budgets until pinned CI hardware exists, the
synthetic corpus is not a substitute for an independently maintained external
evaluation set, and canary/rollback exercise requires an actual release channel.
These limitations must remain visible rather than being represented as code-complete
release operations.
