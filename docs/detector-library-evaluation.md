# Detector Library Evaluation

This records the June 30, 2026 benchmark that compared the built-in detector with Rust PII crates. The benchmark harness was intentionally removed after it answered the adoption question, so these crates do not stay in the workspace dependency graph.

## Result

| Detector | TP | FP | TN | FN | Precision | Recall |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Current detector | 9 | 0 | 3 | 0 | 1.000 | 1.000 |
| `pii` default, no model features | 8 | 2 | 1 | 1 | 0.800 | 0.889 |
| `redact-core` pattern engine | 3 | 1 | 2 | 6 | 0.750 | 0.333 |
| `pii-vault` pattern recognizers only | 5 | 1 | 2 | 4 | 0.833 | 0.556 |

The fixture set covered multilingual headers, structured values, contextual Dutch BTW, EU VAT checksum near-misses, and ambiguous benign headers.

## Conclusion

The current detector wins the semantics that matter for this app: table headers, sample-value context, VAT/BTW validation, conservative false-positive behavior, and traceable decisions. It should remain the default production path.

`pii` is the only crate worth reconsidering in a future spike because its deterministic analyzer, byte offsets, recognizers, and overlap decision model align with parts of this project. It still lost important fixture semantics in this evaluation, so it should not be adopted without a clear fixture win and acceptable dependency impact.

`redact-core` and `pii-vault` are not worth keeping as dependencies. `redact-core` is more useful as a design reference than as a dependency here, and `pii-vault` centers reversible vault/tokenization behavior that this app intentionally avoids.

Model-backed detectors, cloud DLP paths, reversible vault/token systems, and generic text scrubbers do not belong in the default detector path. If explored later, they should be opt-in and measured against the multilingual fixture matrix before any product claim changes.
