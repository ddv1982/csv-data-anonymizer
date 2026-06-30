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
