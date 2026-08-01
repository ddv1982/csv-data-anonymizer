# ADR 001: Typed IPC boundary

Status: accepted

Frontend command functions accept named request objects matching Rust DTOs. Tauri commands return a structured `CommandError` with a stable code, safe message, optional remedy, and retryability flag.

Positional command APIs and UI control flow based on matching error prose are not allowed. Internal Rust helpers may use narrower error representations; conversion happens once at the command boundary.
