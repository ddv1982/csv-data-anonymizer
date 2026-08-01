# Repeatable keyed tokenization

Tokenization remains randomized by default. A user may optionally supply a 256-bit key as 64
hexadecimal characters for one application session. With that option enabled, the same normalized
source value, column position, column name, and key produce the same `tok_` value across runs.

The construction uses keyed BLAKE3 with the domain label `csv-anonymizer/keyed-token/v1`. Column
position and name are included deliberately: equality is preserved within a column and across
releases made with the same schema and key, but equal values in unrelated columns do not receive
the same token.

## Secret lifecycle

- The frontend keeps the key in React state only. It is not part of `AppSettings` and is lost when
  the application exits or reloads.
- IPC request types accept it only for preview, paste transform, quick generation, and the
  background CSV job. Rust request types carrying the raw string do not implement `Debug`.
- The core validates the string into `TokenizationKey`, a type with no serialization
  implementation and a redacted `Debug` implementation.
- Prepared-analysis snapshots, job status, privacy reports, release-context exports, errors, and
  settings never contain the key or a reusable fingerprint of it.

Reusing a key deliberately makes tokenized values linkable across releases. Losing the key makes
the same tokens impossible to reproduce. The key is therefore release material that the user must
store separately if repeatability is required.
