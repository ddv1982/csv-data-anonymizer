# Robust semantic redaction

## Contract

Detection keeps three decisions separate:

1. **Format** describes value structure, such as UUID, email or timestamp.
2. **Privacy meaning** describes what the value represents, such as a person,
   persistent identifier or network/device identifier.
3. **Strategy** describes the transformation.

A strong format match is not automatically strong semantic evidence. In particular,
a UUID is a persistent identifier whose subject is unknown. It remains Medium risk,
is selected automatically, and defaults to Redact. Only independent device-specific
header context narrows a UUID to a network/device identifier. IP and MAC formats
continue to provide network/device evidence directly.

## Redact and Label

Redact never retains a distinct-value mapping:

```text
UUID A -> [CUSTOM_REFERENCE]
UUID B -> [CUSTOM_REFERENCE]
UUID A -> [CUSTOM_REFERENCE]
```

Reliable semantic evidence may instead use a typed marker such as `[PERSON]`,
`[EMAIL]`, `[ADDRESS]` or `[NETWORK_ID]`. When semantics are unresolved, the marker
is a normalized form of the already-published header. Duplicate normalized headers
are qualified with their column index. Blank or punctuation-only headers fall back
to `[COLUMN_n]`.

Label remains an explicit, linkable pseudonymization strategy:

```text
UUID A -> [CUSTOM_REFERENCE_1]
UUID B -> [CUSTOM_REFERENCE_2]
UUID A -> [CUSTOM_REFERENCE_1]
```

It is never selected automatically.

## Evidence and interface

The review table calls the structural result **Detected Format**. Privacy evidence
is shown separately and the neutral `RecordIdentifier` kind is displayed as
**Persistent identifier**. For selected Redact columns, the table previews the exact
constant output marker.

## Calibration and release gates

The regression corpus covers generic UUID record identifiers and independently
supported device identifiers without copying user data into product logic. Release
gates are:

- generic UUIDs remain Medium risk, selected, and Redact by default;
- UUID format alone never produces network/device evidence;
- device-specific header context can refine a UUID to network/device;
- IP and MAC behavior is unchanged;
- uncertain Redact output is constant and stores no value mapping;
- Label numbering occurs only under the explicit Label strategy;
- typed placeholders require actionable semantic evidence;
- duplicate, blank, punctuation-only and Unicode headers produce deterministic
  safe markers;
- preview and final transformations use the same backend strategy;
- Rust, frontend, lint, typecheck, and production build checks pass.

The detector quality suite should continue measuring format accuracy, semantic
accuracy, sensitive-column recall, false auto-selection, and incorrect typed-marker
rate independently. Unsafe pass-through and preview/output disagreement remain
zero-tolerance failures.

The broader evidence-profile, calibration, performance, compatibility, and rollout
work is tracked in
[`semantic-redaction-program-status.md`](semantic-redaction-program-status.md).
That document is status-oriented: it does not treat planned exit gates as already
implemented.
