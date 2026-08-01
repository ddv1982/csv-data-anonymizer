# ADR 005: Module boundaries over line-count targets

Status: accepted

Modules are split when a responsibility has a distinct dependency direction, security boundary, or reason to change. Line count alone is not a boundary.

The IPC error contract, job transport, release-context UI, and explicit run-secret plumbing live in dedicated modules because they satisfy that test. Large privacy-calculation modules remain cohesive when their invariants and tests depend on shared internal state. Contract DTO declarations may remain aggregated while the contract checker indexes their source; behavior must not accumulate in that declaration module.

Future extraction must be behavior-preserving, reduce visibility where possible, and land independently from changes to privacy semantics. Creating more crates requires a deployment, dependency, or reuse boundary—not merely a shorter file.
