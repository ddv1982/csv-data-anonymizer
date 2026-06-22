use super::state::{PseudonymDomain, TransformState};

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

pub(super) fn transform_first_name(value: &str, state: &mut TransformState) -> String {
    let excluded_tokens: Vec<&str> = value.split_whitespace().collect();
    transform_name_tokens(
        value,
        state,
        PseudonymDomain::FirstName,
        FIRST_NAMES,
        &excluded_tokens,
    )
}

pub(super) fn transform_last_name(value: &str, state: &mut TransformState) -> String {
    let excluded_tokens: Vec<&str> = value.split_whitespace().collect();
    transform_name_tokens(
        value,
        state,
        PseudonymDomain::LastName,
        LAST_NAMES,
        &excluded_tokens,
    )
}

pub(super) fn transform_full_name(value: &str, state: &mut TransformState) -> String {
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
