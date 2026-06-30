use crate::smart::SmartReplacementMap;
use crate::types::TransformReport;
use rand::Rng;
use std::collections::HashMap;

const GENERATED_ATTEMPT_LIMIT: usize = 512;
pub(super) const TOKEN_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz0123456789";
pub(super) const LETTER_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz";

#[derive(Debug, Clone, Default)]
pub struct TransformState {
    mappers: HashMap<PseudonymDomain, PseudonymMapper>,
    smart_replacements: SmartReplacementMap,
    report: TransformReport,
}

impl TransformState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_smart_replacements(smart_replacements: SmartReplacementMap) -> Self {
        let smart_replacement_values = smart_replacements.len();
        let smart_replacement_requests = smart_replacements.requested_values();
        let smart_replacement_rejections = smart_replacements.rejected_values();
        let smart_replacement_rejection_reasons = smart_replacements.rejection_reasons();
        Self {
            mappers: HashMap::new(),
            smart_replacements,
            report: TransformReport {
                smart_replacement_requests,
                smart_replacement_values,
                smart_replacement_rejections,
                smart_replacement_rejection_reasons,
                ..TransformReport::default()
            },
        }
    }

    pub fn report(&self) -> TransformReport {
        self.report.clone()
    }

    fn mapper_mut(&mut self, domain: PseudonymDomain) -> &mut PseudonymMapper {
        self.mappers.entry(domain).or_default()
    }

    pub(super) fn assign_from_pool(
        &mut self,
        domain: PseudonymDomain,
        value: &str,
        candidates: &[&str],
        excluded_tokens: &[&str],
    ) -> String {
        let source_key = normalized_identity(value);
        if let Some(existing) = self
            .mapper_mut(domain)
            .source_to_output
            .get(&source_key)
            .cloned()
        {
            self.report.reused_pseudonym_values += 1;
            return existing;
        }

        let start_index = rand::thread_rng().gen_range(0..candidates.len());
        let mut collided = false;

        for offset in 0..candidates.len() {
            let candidate = candidates[(start_index + offset) % candidates.len()];
            if excluded_tokens
                .iter()
                .any(|token| candidate.eq_ignore_ascii_case(token.trim()))
            {
                continue;
            }
            if self.output_is_used_by_other_source(domain, candidate, &source_key) {
                collided = true;
                continue;
            }

            return self.register_assignment(domain, &source_key, candidate.to_string(), collided);
        }

        self.report.exhausted_pseudonym_pools += 1;
        for attempt in 0..GENERATED_ATTEMPT_LIMIT {
            let base = candidates[(start_index + attempt) % candidates.len()];
            let suffix = generated_name_suffix();
            let candidate = format!("{base}{suffix}");
            if excluded_tokens
                .iter()
                .any(|token| candidate.eq_ignore_ascii_case(token.trim()))
            {
                continue;
            }
            if !self.output_is_used_by_other_source(domain, &candidate, &source_key) {
                return self.register_assignment(domain, &source_key, candidate, collided);
            }
        }

        let fallback = format!("{}{}", candidates[start_index], generated_name_suffix());
        self.register_exhausted_assignment(domain, &source_key, fallback)
    }

    pub(super) fn assign_generated(
        &mut self,
        domain: PseudonymDomain,
        source_key: &str,
        mut generate: impl FnMut(usize) -> String,
    ) -> String {
        if let Some(existing) = self
            .mapper_mut(domain)
            .source_to_output
            .get(source_key)
            .cloned()
        {
            self.report.reused_pseudonym_values += 1;
            return existing;
        }

        let mut collided = false;
        for attempt in 0..GENERATED_ATTEMPT_LIMIT {
            let candidate = generate(attempt);
            if candidate.is_empty() {
                continue;
            }
            if self.output_is_used_by_other_source(domain, &candidate, source_key) {
                collided = true;
                continue;
            }

            return self.register_assignment(domain, source_key, candidate, collided);
        }

        self.report.exhausted_pseudonym_pools += 1;
        self.register_exhausted_assignment(domain, source_key, generate(GENERATED_ATTEMPT_LIMIT))
    }

    fn output_is_used_by_other_source(
        &mut self,
        domain: PseudonymDomain,
        candidate: &str,
        source_key: &str,
    ) -> bool {
        self.mapper_mut(domain)
            .output_to_source
            .get(candidate)
            .is_some_and(|owner| owner != source_key)
    }

    fn register_assignment(
        &mut self,
        domain: PseudonymDomain,
        source_key: &str,
        output: String,
        collided: bool,
    ) -> String {
        let mapper = self.mapper_mut(domain);
        mapper
            .source_to_output
            .insert(source_key.to_string(), output.clone());
        mapper
            .output_to_source
            .insert(output.clone(), source_key.to_string());
        self.report.unique_pseudonym_values += 1;
        if collided {
            self.report.collisions_avoided += 1;
        }
        if domain == PseudonymDomain::OpaqueToken {
            self.report.opaque_token_values += 1;
        }
        output
    }

    fn register_exhausted_assignment(
        &mut self,
        domain: PseudonymDomain,
        source_key: &str,
        output: String,
    ) -> String {
        let mapper = self.mapper_mut(domain);
        mapper
            .source_to_output
            .insert(source_key.to_string(), output.clone());
        mapper
            .output_to_source
            .entry(output.clone())
            .or_insert_with(|| source_key.to_string());
        self.report.unique_pseudonym_values += 1;
        if domain == PseudonymDomain::OpaqueToken {
            self.report.opaque_token_values += 1;
        }
        output
    }

    pub(super) fn smart_replacement(&mut self, column_index: usize, value: &str) -> Option<String> {
        self.smart_replacements
            .get(column_index, value)
            .map(ToString::to_string)
    }

    pub(super) fn record_smart_fallback(&mut self) {
        self.report.smart_replacement_fallbacks += 1;
    }
}

#[derive(Debug, Clone, Default)]
struct PseudonymMapper {
    source_to_output: HashMap<String, String>,
    output_to_source: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum PseudonymDomain {
    EmailLocal,
    Uuid,
    Timestamp,
    NumericId,
    NumericValue,
    Phone,
    FirstName,
    LastName,
    GenericString,
    OpaqueToken,
}

pub(super) fn normalized_identity(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub(super) fn random_string(length: usize, charset: &str) -> String {
    let chars: Vec<char> = charset.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| chars[rng.gen_range(0..chars.len())])
        .collect()
}

fn generated_name_suffix() -> String {
    random_string(4, LETTER_CHARSET)
}
