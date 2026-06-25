use crate::csv_io::read_csv_sample_from_str;
use crate::types::PasteDataFormat;
use serde_json::Value;

use super::documents::{parse_json, parse_yaml};
use super::text::looks_like_logs;
use super::xml::collect_xml_fields;

pub(super) fn resolve_format(requested: PasteDataFormat, content: &str) -> PasteDataFormat {
    if requested != PasteDataFormat::Auto {
        return requested;
    }
    detect_paste_format(content)
}

fn detect_paste_format(content: &str) -> PasteDataFormat {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return PasteDataFormat::PlainText;
    }
    if parse_json(trimmed).is_ok() {
        return PasteDataFormat::Json;
    }
    if trimmed.starts_with('<') && collect_xml_fields(trimmed, 1).is_ok() {
        return PasteDataFormat::Xml;
    }
    if looks_like_csv(trimmed) {
        return PasteDataFormat::Csv;
    }
    if looks_like_yaml(trimmed)
        && parse_yaml(trimmed).is_ok_and(|value| !matches!(value, Value::String(_)))
    {
        return PasteDataFormat::Yaml;
    }
    if looks_like_logs(trimmed) {
        return PasteDataFormat::Logs;
    }
    PasteDataFormat::PlainText
}

fn looks_like_csv(content: &str) -> bool {
    let Some(first_line) = content.lines().find(|line| !line.trim().is_empty()) else {
        return false;
    };
    if !first_line.contains(',') {
        return false;
    }
    read_csv_sample_from_str(content, 1)
        .map(|sample| sample.headers.len() > 1)
        .unwrap_or(false)
}

fn looks_like_yaml(content: &str) -> bool {
    content.contains(":\n")
        || content.lines().any(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("- ") || trimmed.contains(": ")
        })
}
