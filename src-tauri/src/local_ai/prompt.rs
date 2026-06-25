use csv_anonymizer_core::SmartReplacementRequest;
use serde_json::{Value, json};

pub(super) fn smart_replacement_prompt(request: SmartReplacementRequest<'_>) -> String {
    let values = serde_json::to_string(request.values).unwrap_or_else(|_| "[]".to_string());
    format!(
        "Create realistic fake CSV replacement values. Data stays local. Return only JSON matching the schema. Do not copy any original value, do not include personal data, and keep the same broad data type. Column name: {name}. Detected type: {data_type:?}. Values JSON array: {values}",
        name = request.column.name,
        data_type = request.column.detected_type,
    )
}

pub(super) fn replacement_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "replacements": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "original": { "type": "string" },
                        "replacement": { "type": "string" }
                    },
                    "required": ["original", "replacement"]
                }
            }
        },
        "required": ["replacements"]
    })
}

pub(super) fn stable_seed(seed: &str, column_index: usize) -> u64 {
    let mut hash = 14_695_981_039_346_656_037_u64;
    for byte in format!("{seed}:{column_index}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash & 0x7fff_ffff
}
