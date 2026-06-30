# Dependency Audit Follow-Ups

Last reviewed: 2026-06-30

This project treats `cargo audit` warnings as review inputs, not automatic
release blockers. As of this review, `cargo audit --json` reports zero
vulnerabilities and several informational warnings.

## RustSec Warnings To Monitor

- `anyhow`: lockfile updated from `1.0.102` to `1.0.103`. Re-run
  `npm run cargo:audit` after future RustSec database updates; the advisory may
  continue to warn while RustSec has no patched-version range.
- GTK3 / GLib stack: warnings come through Tauri's Linux webview/runtime path
  (`tauri -> wry/tao/muda/webkit2gtk -> gtk/glib`). They do not appear in the
  macOS or Windows target trees. Monitor Tauri/wry releases for GTK4/WebKit
  migration paths before treating this as an app-level refactor.
- `atomic-polyfill`: transitive through
  `phonenumber -> postcard -> heapless -> atomic-polyfill`. Keep
  `phonenumber` unless a maintained validator with comparable metadata and API
  quality is available.
- `unic-*`: transitive through `urlpattern -> tauri-utils`. Track with Tauri
  updates rather than overriding these crates locally.

## Complexity Refactors To Schedule Separately

The multilingual detection work removed the detection-specific
`clippy::too_many_lines` warnings. Two unrelated functions still exceed the
optional line-count lint:

- `crates/csv-anonymizer-core/src/direct_input/quick.rs`: `generated_quick_value`
- `crates/csv-anonymizer-core/src/service.rs`: `preflight_anonymization`

Refactor these in separate changes so behavioral detection updates remain
reviewable.
