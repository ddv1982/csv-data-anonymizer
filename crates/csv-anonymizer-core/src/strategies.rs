use crate::detection::is_empty_value;
use crate::hash::{deterministic_number, deterministic_string, deterministic_uuid};
use crate::smart::SmartReplacementMap;
use crate::types::{
    AnonymizationStrategy, ColumnMetadata, DataType, TransformContext, TransformReport,
};
use chrono::{Duration, NaiveDate};
use rand::Rng;
use std::collections::HashMap;

const FIRST_NAMES: &[&str] = &[
    "Adam",
    "Adrian",
    "Aiden",
    "Alex",
    "Amelia",
    "Andrew",
    "Ari",
    "Ariana",
    "Audrey",
    "Austin",
    "Bailey",
    "Blake",
    "Brianna",
    "Caleb",
    "Camila",
    "Cameron",
    "Casey",
    "Charlotte",
    "Chloe",
    "Claire",
    "Cole",
    "Connor",
    "Dana",
    "Daniel",
    "Dylan",
    "Eleanor",
    "Elena",
    "Eli",
    "Elijah",
    "Elliot",
    "Emery",
    "Emma",
    "Ethan",
    "Evelyn",
    "Felix",
    "Finley",
    "Gabriel",
    "Grace",
    "Hannah",
    "Harper",
    "Isaac",
    "Isabella",
    "Ivy",
    "Jack",
    "Jade",
    "Jamie",
    "Jasmine",
    "Jordan",
    "Julia",
    "Kai",
    "Layla",
    "Leo",
    "Liam",
    "Logan",
    "Lucas",
    "Maya",
    "Mia",
    "Miles",
    "Naomi",
    "Nora",
    "Olivia",
    "Owen",
    "Parker",
    "Quinn",
    "Reese",
    "Riley",
    "Rowan",
    "Ryan",
    "Sam",
    "Sofia",
    "Taylor",
    "Theo",
    "Violet",
    "Willow",
    "Wyatt",
    "Zoe",
];
const LAST_NAMES: &[&str] = &[
    "Adams",
    "Anderson",
    "Baker",
    "Bennett",
    "Brooks",
    "Brown",
    "Campbell",
    "Carter",
    "Clark",
    "Coleman",
    "Collins",
    "Cooper",
    "Cruz",
    "Davis",
    "Diaz",
    "Edwards",
    "Evans",
    "Fisher",
    "Flores",
    "Foster",
    "Garcia",
    "Gomez",
    "Gray",
    "Green",
    "Hall",
    "Hayes",
    "Henderson",
    "Hill",
    "Howard",
    "Hughes",
    "Jackson",
    "James",
    "Jenkins",
    "Johnson",
    "Kelly",
    "King",
    "Lee",
    "Lewis",
    "Lopez",
    "Martin",
    "Martinez",
    "Miller",
    "Mitchell",
    "Moore",
    "Morgan",
    "Morris",
    "Murphy",
    "Nelson",
    "Nguyen",
    "Parker",
    "Patel",
    "Perez",
    "Phillips",
    "Ramirez",
    "Reed",
    "Rivera",
    "Roberts",
    "Robinson",
    "Rodriguez",
    "Ross",
    "Russell",
    "Sanchez",
    "Scott",
    "Simmons",
    "Smith",
    "Stewart",
    "Sullivan",
    "Taylor",
    "Thomas",
    "Thompson",
    "Torres",
    "Turner",
    "Walker",
    "Ward",
    "Watson",
    "White",
    "Williams",
    "Wilson",
    "Wood",
    "Wright",
    "Young",
];

const GENERATED_ATTEMPT_LIMIT: usize = 512;
const TOKEN_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz0123456789";
const LETTER_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz";

#[derive(Debug, Clone)]
pub struct TransformState {
    deterministic: bool,
    seed: String,
    mappers: HashMap<PseudonymDomain, PseudonymMapper>,
    smart_replacements: SmartReplacementMap,
    report: TransformReport,
}

impl TransformState {
    pub fn new(deterministic: bool, seed: impl Into<String>) -> Self {
        Self {
            deterministic,
            seed: seed.into(),
            mappers: HashMap::new(),
            smart_replacements: SmartReplacementMap::default(),
            report: TransformReport::default(),
        }
    }

    pub fn with_smart_replacements(
        deterministic: bool,
        seed: impl Into<String>,
        smart_replacements: SmartReplacementMap,
    ) -> Self {
        let smart_replacement_values = smart_replacements.len();
        Self {
            deterministic,
            seed: seed.into(),
            mappers: HashMap::new(),
            smart_replacements,
            report: TransformReport {
                smart_replacement_values,
                ..TransformReport::default()
            },
        }
    }

    pub fn report(&self) -> TransformReport {
        self.report
    }

    fn mapper_mut(&mut self, domain: PseudonymDomain) -> &mut PseudonymMapper {
        self.mappers.entry(domain).or_default()
    }

    fn assign_from_pool(
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

        let start_index = if self.deterministic {
            deterministic_number(
                &source_key,
                &format!("{}:{}:pool", self.seed, domain.seed_key()),
                0,
                candidates.len() as i64 - 1,
            ) as usize
        } else {
            rand::thread_rng().gen_range(0..candidates.len())
        };
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
            let suffix =
                generated_name_suffix(&source_key, &self.seed, domain, attempt, self.deterministic);
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

        let fallback = format!(
            "{}{}",
            candidates[start_index],
            generated_name_suffix(
                &source_key,
                &self.seed,
                domain,
                GENERATED_ATTEMPT_LIMIT,
                self.deterministic,
            )
        );
        self.register_exhausted_assignment(domain, &source_key, fallback)
    }

    fn assign_generated(
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

    fn smart_replacement(&mut self, column_index: usize, value: &str) -> Option<String> {
        self.smart_replacements
            .get(column_index, value)
            .map(ToString::to_string)
    }

    fn record_smart_fallback(&mut self) {
        self.report.smart_replacement_fallbacks += 1;
    }
}

#[derive(Debug, Clone, Default)]
struct PseudonymMapper {
    source_to_output: HashMap<String, String>,
    output_to_source: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PseudonymDomain {
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

impl PseudonymDomain {
    fn seed_key(self) -> &'static str {
        match self {
            PseudonymDomain::EmailLocal => "email-local",
            PseudonymDomain::Uuid => "uuid",
            PseudonymDomain::Timestamp => "timestamp",
            PseudonymDomain::NumericId => "numeric-id",
            PseudonymDomain::NumericValue => "numeric-value",
            PseudonymDomain::Phone => "phone",
            PseudonymDomain::FirstName => "first-name",
            PseudonymDomain::LastName => "last-name",
            PseudonymDomain::GenericString => "generic-string",
            PseudonymDomain::OpaqueToken => "opaque-token",
        }
    }
}

pub fn transform_value(
    value: &str,
    column: &ColumnMetadata,
    context: &TransformContext<'_>,
) -> String {
    let mut state = TransformState::new(context.deterministic, context.seed);
    transform_value_with_state(value, column, context, &mut state)
}

pub fn transform_value_with_state(
    value: &str,
    column: &ColumnMetadata,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    if is_empty_value(value) {
        return value.to_string();
    }

    match column.strategy {
        AnonymizationStrategy::PassThrough => return value.to_string(),
        AnonymizationStrategy::Mask => return mask_value(value),
        AnonymizationStrategy::Tokenize => return transform_opaque_token(value, context, state),
        AnonymizationStrategy::LocalAi => {
            if let Some(replacement) = state.smart_replacement(column.index, value) {
                return replacement;
            }
            state.record_smart_fallback();
        }
        AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {}
    }

    match column.detected_type {
        DataType::Email => transform_email(value, context, state),
        DataType::Uuid => transform_uuid(value, context, state),
        DataType::Timestamp => transform_timestamp(value, context, state),
        DataType::NumericId => transform_numeric_id(value, context, state),
        DataType::NumericValue => transform_numeric_value(value, context, state),
        DataType::Phone => transform_phone(value, context, state),
        DataType::FirstName => transform_first_name(value, state),
        DataType::LastName => transform_last_name(value, state),
        DataType::FullName => transform_full_name(value, state),
        DataType::PostalCode
        | DataType::Address
        | DataType::IpAddress
        | DataType::Url
        | DataType::MacAddress
        | DataType::TaxId
        | DataType::String
        | DataType::Unknown => transform_generic_string(value, context, state),
        DataType::Boolean | DataType::Currency | DataType::Percentage => value.to_string(),
        DataType::CountryCode | DataType::Enum => value.to_string(),
    }
}

fn mask_value(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_whitespace() {
                character
            } else {
                '*'
            }
        })
        .collect()
}

pub fn transform_row(
    row: &[String],
    columns: &[ColumnMetadata],
    row_index: usize,
    seed: &str,
    deterministic: bool,
) -> Vec<String> {
    let mut state = TransformState::new(deterministic, seed);
    transform_row_with_state(row, columns, row_index, seed, deterministic, &mut state)
}

pub fn transform_row_with_state(
    row: &[String],
    columns: &[ColumnMetadata],
    row_index: usize,
    seed: &str,
    deterministic: bool,
    state: &mut TransformState,
) -> Vec<String> {
    row.iter()
        .enumerate()
        .map(|(column_index, value)| {
            let Some(column) = columns.get(column_index) else {
                return value.clone();
            };

            if !column.is_selected {
                return value.clone();
            }

            let context = TransformContext {
                column_name: &column.name,
                column_index: column.index,
                row_index,
                seed,
                deterministic,
                empty_format: column.empty_format,
            };
            transform_value_with_state(value, column, &context, state)
        })
        .collect()
}

fn transform_opaque_token(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = format!(
        "{}:{}:{}",
        context.column_name,
        context.column_index,
        normalized_identity(value)
    );
    state.assign_generated(PseudonymDomain::OpaqueToken, &source_key, |attempt| {
        if context.deterministic {
            format!(
                "tok_{}",
                deterministic_string(
                    &source_key,
                    &format!("{}:opaque:{attempt}", context.seed),
                    16,
                    TOKEN_CHARSET,
                )
            )
        } else {
            format!("tok_{}", random_string(16, TOKEN_CHARSET))
        }
    })
}

fn transform_email(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let Some(at_index) = value.rfind('@') else {
        return value.to_string();
    };
    let domain = &value[at_index..];
    let source_key = normalized_identity(value);
    let local_part = state.assign_generated(PseudonymDomain::EmailLocal, &source_key, |attempt| {
        if context.deterministic {
            let prefix = deterministic_string(
                value,
                &format!("{}:email-prefix:{attempt}", context.seed),
                6,
                LETTER_CHARSET,
            );
            let suffix = deterministic_string(
                value,
                &format!("{}:email-suffix:{attempt}", context.seed),
                3,
                "0123456789",
            );
            format!("{prefix}{suffix}")
        } else {
            let mut rng = rand::thread_rng();
            format!("user{}", rng.gen_range(1..=999_999))
        }
    });
    format!("{local_part}{domain}")
}

fn transform_uuid(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = normalized_identity(value);
    let uuid = state.assign_generated(PseudonymDomain::Uuid, &source_key, |attempt| {
        if context.deterministic {
            deterministic_uuid(value, &format!("{}:uuid:{attempt}", context.seed))
        } else {
            random_uuid_v4()
        }
    });
    if value == value.to_uppercase() {
        uuid.to_uppercase()
    } else {
        uuid
    }
}

fn random_uuid_v4() -> String {
    let mut bytes = [0_u8; 16];
    rand::thread_rng().fill(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn transform_timestamp(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = normalized_identity(value);
    state.assign_generated(PseudonymDomain::Timestamp, &source_key, |attempt| {
        transform_timestamp_candidate(value, context, attempt)
    })
}

fn transform_timestamp_candidate(
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    if value.len() < 10 {
        return value.to_string();
    }

    let Ok(date) = NaiveDate::parse_from_str(&value[..10], "%Y-%m-%d") else {
        return value.to_string();
    };

    let offset_days = if context.deterministic {
        deterministic_number(
            value,
            &format!("{}:timestamp:{attempt}", context.seed),
            -365,
            365,
        )
    } else {
        rand::thread_rng().gen_range(-365..=365)
    };

    let Some(offset_date) = date.checked_add_signed(Duration::days(offset_days)) else {
        return value.to_string();
    };

    format!("{}{}", offset_date.format("%Y-%m-%d"), &value[10..])
}

fn transform_numeric_id(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = format!("{}:{}", value.len(), value);
    state.assign_generated(PseudonymDomain::NumericId, &source_key, |attempt| {
        transform_numeric_id_candidate(value, context, attempt)
    })
}

fn transform_numeric_id_candidate(
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    let digit_count = value.len();
    if digit_count == 0 {
        return value.to_string();
    }

    let leading_zero_count = value
        .chars()
        .take_while(|character| *character == '0')
        .count();
    if leading_zero_count > 0 && leading_zero_count < digit_count {
        let generated =
            generate_numeric_id(digit_count - leading_zero_count, value, context, attempt);
        return format!("{}{}", "0".repeat(leading_zero_count), generated);
    }

    if leading_zero_count == digit_count {
        return generate_zero_width_numeric_id(digit_count, value, context, attempt);
    }

    generate_numeric_id(digit_count, value, context, attempt)
}

fn generate_zero_width_numeric_id(
    length: usize,
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    if context.deterministic {
        deterministic_string(
            value,
            &format!("{}:zero:{attempt}", context.seed),
            length,
            "0123456789",
        )
    } else {
        let mut rng = rand::thread_rng();
        (0..length)
            .map(|_| rng.gen_range(0..=9).to_string())
            .collect()
    }
}

fn generate_numeric_id(
    length: usize,
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    if context.deterministic {
        let first_digit = deterministic_string(
            value,
            &format!("{}:first:{attempt}", context.seed),
            1,
            "123456789",
        );
        if length == 1 {
            return first_digit;
        }
        let rest_digits = deterministic_string(
            value,
            &format!("{}:rest:{attempt}", context.seed),
            length - 1,
            "0123456789",
        );
        format!("{first_digit}{rest_digits}")
    } else {
        let mut rng = rand::thread_rng();
        let first_digit = rng.gen_range(1..=9).to_string();
        let rest: String = (1..length)
            .map(|_| rng.gen_range(0..=9).to_string())
            .collect();
        format!("{first_digit}{rest}")
    }
}

fn transform_numeric_value(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = format!("{}:{}", value.len(), value);
    state.assign_generated(PseudonymDomain::NumericValue, &source_key, |attempt| {
        transform_numeric_value_candidate(value, context, attempt)
    })
}

fn transform_numeric_value_candidate(
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    let (sign, unsigned) = match value.as_bytes().first() {
        Some(b'+') | Some(b'-') => (&value[..1], &value[1..]),
        _ => ("", value),
    };

    let Some((integer_part, fractional_part)) = unsigned.split_once('.') else {
        return format!(
            "{sign}{}",
            generate_numeric_component(unsigned, value, context, attempt)
        );
    };

    let integer = generate_numeric_component(integer_part, value, context, attempt);
    let fraction = generate_fractional_component(fractional_part, value, context, attempt);

    format!("{sign}{integer}.{fraction}")
}

fn generate_numeric_component(
    component: &str,
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    if component.is_empty() {
        return String::new();
    }

    let leading_zero_count = component
        .chars()
        .take_while(|character| *character == '0')
        .count();
    if leading_zero_count == component.len() {
        return component.to_string();
    }

    let generated = generate_numeric_id(
        component.len() - leading_zero_count,
        value,
        context,
        attempt,
    );
    format!("{}{}", "0".repeat(leading_zero_count), generated)
}

fn generate_fractional_component(
    component: &str,
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    if component.is_empty() {
        return String::new();
    }

    if context.deterministic {
        deterministic_string(
            value,
            &format!("{}:fraction:{attempt}", context.seed),
            component.len(),
            "0123456789",
        )
    } else {
        let mut rng = rand::thread_rng();
        (0..component.len())
            .map(|_| rng.gen_range(0..=9).to_string())
            .collect()
    }
}

fn transform_phone(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = normalized_identity(value);
    state.assign_generated(PseudonymDomain::Phone, &source_key, |attempt| {
        transform_phone_candidate(value, context, attempt)
    })
}

fn transform_phone_candidate(
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    let mut digit_index = 0;
    value
        .chars()
        .map(|character| {
            if !character.is_ascii_digit() {
                return character.to_string();
            }

            let seed = format!("{}:phone:{attempt}:{digit_index}", context.seed);
            digit_index += 1;
            if context.deterministic {
                deterministic_string(value, &seed, 1, "0123456789")
            } else {
                rand::thread_rng().gen_range(0..=9).to_string()
            }
        })
        .collect()
}

fn transform_first_name(value: &str, state: &mut TransformState) -> String {
    let excluded_tokens: Vec<&str> = value.split_whitespace().collect();
    transform_name_tokens(
        value,
        state,
        PseudonymDomain::FirstName,
        FIRST_NAMES,
        &excluded_tokens,
    )
}

fn transform_last_name(value: &str, state: &mut TransformState) -> String {
    let excluded_tokens: Vec<&str> = value.split_whitespace().collect();
    transform_name_tokens(
        value,
        state,
        PseudonymDomain::LastName,
        LAST_NAMES,
        &excluded_tokens,
    )
}

fn transform_full_name(value: &str, state: &mut TransformState) -> String {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    let token_count = tokens.len();
    if token_count <= 1 {
        return transform_first_name(value, state);
    }

    let first = choose_name_excluding(
        tokens[0],
        state,
        PseudonymDomain::FirstName,
        FIRST_NAMES,
        &tokens,
    );
    let last = tokens[1..]
        .iter()
        .map(|token| {
            choose_name_excluding(token, state, PseudonymDomain::LastName, LAST_NAMES, &tokens)
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("{first} {last}")
}

fn transform_name_tokens(
    value: &str,
    state: &mut TransformState,
    domain: PseudonymDomain,
    names: &[&str],
    excluded_tokens: &[&str],
) -> String {
    value
        .split_whitespace()
        .map(|token| choose_name_excluding(token, state, domain, names, excluded_tokens))
        .collect::<Vec<_>>()
        .join(" ")
}

fn choose_name_excluding(
    value: &str,
    state: &mut TransformState,
    domain: PseudonymDomain,
    names: &[&str],
    excluded_tokens: &[&str],
) -> String {
    state.assign_from_pool(domain, value, names, excluded_tokens)
}

fn transform_generic_string(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = format!("{}:{}", value.len(), normalized_identity(value));
    state.assign_generated(PseudonymDomain::GenericString, &source_key, |attempt| {
        transform_generic_string_candidate(value, context, attempt)
    })
}

fn transform_generic_string_candidate(
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    let target_length = value.len();
    if target_length == 0 {
        return value.to_string();
    }

    let min_length = (target_length as f64 * 0.8).floor().max(1.0) as usize;
    let max_length = (target_length as f64 * 1.2).ceil() as usize;
    let charset = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";

    let output_length = if context.deterministic {
        deterministic_number(
            value,
            &format!("{}:length:{attempt}", context.seed),
            min_length as i64,
            max_length as i64,
        ) as usize
    } else {
        rand::thread_rng().gen_range(min_length..=max_length)
    };

    if context.deterministic {
        deterministic_string(
            value,
            &format!("{}:content:{attempt}", context.seed),
            output_length,
            charset,
        )
    } else {
        let chars: Vec<char> = charset.chars().collect();
        let mut rng = rand::thread_rng();
        (0..output_length)
            .map(|_| chars[rng.gen_range(0..chars.len())])
            .collect()
    }
}

fn normalized_identity(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn random_string(length: usize, charset: &str) -> String {
    let chars: Vec<char> = charset.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| chars[rng.gen_range(0..chars.len())])
        .collect()
}

fn generated_name_suffix(
    source_key: &str,
    seed: &str,
    domain: PseudonymDomain,
    attempt: usize,
    deterministic: bool,
) -> String {
    if deterministic {
        deterministic_string(
            source_key,
            &format!("{seed}:{}:fallback:{attempt}", domain.seed_key()),
            4,
            LETTER_CHARSET,
        )
    } else {
        random_string(4, LETTER_CHARSET)
    }
}

#[cfg(test)]
mod tests;
