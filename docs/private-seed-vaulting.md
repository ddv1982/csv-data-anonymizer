# Private Seed Vaulting

Date: 2026-06-29

## Decision

Remembered repeatable-replacement seeds are stored in the local OS credential store through the Rust `keyring` crate. The JSON settings file keeps `seed` empty even when `rememberSeed` is enabled.

This gives the desktop app keychain-style storage without adding a new frontend-facing secret API. The Tauri settings commands already cross the Rust boundary, so keeping vault access inside the existing settings path keeps the attack surface smaller and avoids exposing seed read/write commands to webview code.

## Alternatives Considered

Tauri Stronghold was considered because it provides an encrypted client-side vault. It requires frontend plugin wiring, capability permissions, and a vault password flow. That is useful when the app needs a user-managed encrypted store shared with webview code, but this app only needs to persist one local seed behind existing Rust settings commands.

The OS credential store approach maps directly to macOS Keychain, Windows Credential Manager, and Linux Secret Service through `keyring`, with lower integration risk for this codebase.

## Behavior

- Loading settings migrates a remembered seed from older JSON settings into the credential store and rewrites the settings file with `seed` cleared.
- Saving settings with `rememberSeed` enabled and a non-empty seed writes the seed to the credential store and writes JSON with `seed` cleared.
- Saving settings with `rememberSeed` enabled and a blank seed clears the remembered seed, disables `rememberSeed`, and writes JSON with `seed` cleared. A blank seed never reloads an older vault value.
- Saving settings with `rememberSeed` disabled makes a best-effort attempt to delete the credential-store entry and writes JSON with `seed` cleared, while keeping the current session seed in memory until the user clears it or exits. Credential-store delete failures do not block unrelated settings saves.
- DP budget updates preserve the vault-backed seed by loading and saving settings through the same vault-aware settings path.

## References

- Tauri Stronghold plugin: https://v2.tauri.app/plugin/stronghold/
- Tauri Stronghold JavaScript API: https://v2.tauri.app/reference/javascript/stronghold/
- Rust keyring crate: https://docs.rs/keyring/latest/keyring/
