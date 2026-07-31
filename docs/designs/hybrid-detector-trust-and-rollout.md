# Hybrid detector trust and rollout

Status: implemented with an opt-in Ollama runtime and prepared-analysis replay

## Current implementation boundary

The optional detector uses the configured Ollama model through the loopback API.
It is off by default, refuses recognized Ollama cloud-model names, uses bounded
structured-output requests, and reports unavailable or failed runs without
discarding deterministic results. The application cannot prove how an
independently configured Ollama server routes an unrecognized model.

The model assist selects a deterministic, column-balanced subset when the
candidate population exceeds its cell, prompt-byte, or request budget. Such a
run is reported as incomplete with examined and eligible cell counts; exhausting
a safety budget is not treated as a model failure and is never presented as a
clean full-coverage result. Oversized cells are skipped and counted rather than
truncated, preserving exact evidence offsets.

Model-assisted evidence is analysis/review-only and never auto-selects a column.
A versioned prepared-analysis snapshot binds the source bytes, format, columns,
detector identity, and accepted evidence. Preview and transformation validate
that snapshot instead of rerunning the model; changed sources are rejected and
must be analyzed again. The Tauri process retains the last 16 issued snapshots
and accepts an exact match only, so a renderer cannot manufacture new evidence
by recomputing the snapshot's accidental-corruption checksum.

Selecting a Review-marked column accepts all learned findings in that column as
a group. There is intentionally no per-finding confirmation workflow.

## Decision

A model-assisted detector may add evidence and surface a column for review. It
must not suppress a deterministic finding, change risk or strategy without an
explicit user decision, deselect a column, or turn an incomplete detection pass
into a clean result.

This asymmetric merge is the trust boundary:

1. Run deterministic detection.
2. Run the optional model assist.
3. Keep deterministic risk/action unchanged and add a review marker.
4. Preserve both evidence routes in the trace.
5. If the model is unavailable, invalid, or times out, return the deterministic
   result and record the assist as unavailable.

The assist is valuable only if a locked corpus shows a meaningful safety gain.
The initial gate is at least a two percentage-point increase in sensitive-column
recall, or closure of a named critical coverage gap, with no critical regression
and no more than a one-point increase in benign escalation. If it cannot clear
that gate, retain the simpler deterministic design.

## Evaluation contract

The primary annotation unit is the column because column risk and selection
control the shipped workflow. Mixed/free-text inputs additionally require
cell-level entity labels and exact spans.

Use disjoint development, calibration, locked evaluation, and sentinel sets.
Split by source or generating template, not by individual value. Report:

- sensitive-column precision and recall;
- default-action recall (auto-selected or explicitly surfaced);
- per-entity and per-locale precision and recall;
- benign escalation rate;
- calibration error for model confidence;
- p50/p95 latency, peak memory, and model-unavailable behavior.

Never tune on the locked set. Every confirmed critical false negative becomes a
sentinel after adjudication. A wrong exact type is non-blocking only when the
risk floor and resulting default action remain at least as protective.

## Sampling and failure semantics

Detection quality and detection coverage are separate facts. A detector cannot
receive recall credit for a value it would have recognized outside the sample.
Existing coverage disclosures remain authoritative and must name the examined
and total rows/values. Increasing model confidence does not widen coverage.

The following states are distinct and must not collapse into a low-risk result:

- model unavailable or timed out: deterministic result plus an unavailable note;
- invalid model output: deterministic result plus a rejected-output note;
- disagreement: keep the more protective result and surface review;
- partial sample: preserve the existing partial-coverage disclosure;
- no evidence: unknown/low evidence, never “verified safe”.

No raw input values may be sent to telemetry or retained for rollout analysis.

## Rollout and rollback

Roll out in three steps:

1. Offline evaluation against the locked and sentinel corpora.
2. Shadow mode that records only aggregate, privacy-safe disagreement counters.
3. Guarded activation: first allow the assist to add review findings, then
   enable auto-selection only for entity families that independently pass their
   acceptance gates.

Rollback disables model contribution and restores deterministic output without
a new application release. Roll back immediately for any newly introduced
critical false negative, raw-value telemetry, changed fallback output, a recall
drop greater than one point overall (or two points in a supported entity/locale
slice), a benign-escalation increase greater than two points, or a p95 latency
increase greater than 25%.

## Reporting boundary

Per-file privacy reports may state only the detection basis (`rules`,
`rules + local model`, or `rules; model unavailable`) and actionable
disagreement/unavailability. Corpus scores and calibration charts belong in
release evidence, not an individual file report.

Joint re-identifiability remains an output measurement independent of detector
confidence. The product continues to describe risk reduction, not proof of
anonymity.

## Limits

This design does not guarantee discovery of values outside the sample, novel
proprietary identifiers, contextual or cross-file identifiers, unsupported
languages, obfuscated/encoded/OCR-corrupted values, secrets outside the declared
taxonomy, or relationships visible only across columns. It also does not make a
correctly detected transformation anonymous.
