# ADR 004: Explicit run secrets

Status: accepted

Tokenization keys travel explicitly from the Tauri command through the selected core entry point into `TransformState`. Ambient thread-local secret state is prohibited.

The validated key type deliberately has no serialization implementation and redacts its `Debug` representation. APIs that do not accept run secrets construct a keyless state; callers cannot accidentally inherit a key from unrelated work on the same thread.
