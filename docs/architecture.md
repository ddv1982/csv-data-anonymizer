# Architecture

CSV Anonymizer is a local-first desktop application with three runtime boundaries.

- `csv-anonymizer-core` owns detection, transformation, release evidence, and reusable domain contracts. It has no Tauri dependency.
- `src-tauri` owns filesystem authority, persisted settings, Local AI adapters, background jobs, and IPC translation.
- `frontend` owns interaction state and presentation. It does not infer privacy guarantees that the core can report directly.

## Trust boundaries

- A path is usable only after a picker grant or an explicit confirmation. Read and write grants are separate.
- Prepared analysis is backend-issued and bound to source bytes, detector inputs, and selected columns.
- Tokenization keys are run-only secrets. They are not serializable, persisted, logged, or included in reports.
- Local AI is opt-in, loopback-only, and rejects obvious cloud model forms before making a request.
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

## Change policy

Boundary refactors land separately from behavioral changes. Every phase must pass Rust tests, Clippy, TypeScript, ESLint, frontend tests, IPC contract checks, and browser workflow tests before the next phase begins.
