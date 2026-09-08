# Architecture

CSV Anonymizer is a local-first desktop application with three runtime boundaries. Dated review notes under `docs/` are historical. This file is the current architecture.

- `csv-anonymizer-core` owns detection, transformation, release evidence, and reusable domain contracts. It has no Tauri dependency.
- `src-tauri` owns filesystem authority, persisted settings, Local AI adapters, background jobs, and IPC translation.
- `frontend` owns interaction state and presentation. It does not infer privacy guarantees that the core can report directly.

## Trust boundaries

- A path is usable only after a picker grant or an explicit confirmation. Read and write grants are separate.
- Prepared analysis is backend-issued and bound to source bytes, detector inputs, and selected columns.
- Tokenization keys are run-only secrets. They are not serializable, persisted, logged, or included in reports.
- Local AI is opt-in, loopback-only, and rejects obvious cloud model forms before making a request.
- The packaged webview freezes `Object.prototype` so frontend script cannot patch the object root.
- Output is staged and atomically published. Cancellation or failure must not leave a partial destination.
- Release readiness separates measured technical evidence from user assertions.

## Runtime invariants

1. Changing source bytes or detector inputs invalidates prepared analysis.
2. The same tokenization key and column context produce the same token; different keys separate outputs.
3. At most one anonymization job owns the processing lease.
4. The job registry is authoritative. IPC channels accelerate progress delivery; status queries recover lost channels.
5. Terminal jobs remain queryable long enough for a dropped final update to be recovered.
6. UI workflow phases must not encode contradictory combinations of busy, job, and result state.
7. Showing a preview must preserve the user's reading position.

## Source and workflow boundaries

- Core CSV entry points take explicit execution inputs: `CsvAnalysisOptions` carries sampling and optional candidate detection, `TransformRuntime` carries run-only Smart replacement and tokenization dependencies, and `CsvRunOptions` adds processing control. CLI and Tauri construct those borrowed inputs at their boundary; lower-level processing inputs remain separate.
- `types.rs` remains the internal wire façade. Its declarations are physically owned by `types/column.rs`, `types/sampling.rs`, `types/processing.rs`, `types/workflow.rs`, and `types/report.rs`, while crate-root type paths and serialized contracts remain unchanged.
- Frontend protection readiness is derived by the pure `workflowReadiness` helper. Each CSV, paste, and quick workflow combines it with its own source, selection, and busy prerequisites so disabled controls and direct actions make the same decision.
- `useAnonymizeJob` submits and handles terminal workflow outcomes. `useAnonymizeJobTracker` owns live-channel updates, stale-channel polling, cancellation requests, and lost-contact recovery without changing the parent hook's public shape.

## Change policy

Boundary refactors land separately from behavioral changes. Every phase must pass Rust tests, Clippy, TypeScript, ESLint, frontend tests, IPC contract checks, comment narration checks, and browser workflow tests before the next phase begins.
