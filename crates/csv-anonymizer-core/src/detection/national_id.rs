//! National-ID validation via the idsmith registries.
//! Only checksum-backed schemes are allowlisted; format-only kinds
//! (passports, driver's licenses) match arbitrary IDs too often to vote.

#[derive(Clone, Copy)]
enum Scheme {
    Personal,
    Tax,
}

/// Checksum-backed schemes only, verified by the probe tests in this file.
/// Countries whose probes failed in Task 1 must not appear here.
const ALLOWLIST: &[(&str, Scheme)] = &[
    ("NL", Scheme::Personal),
    ("BE", Scheme::Personal),
    ("PL", Scheme::Personal),
    ("IT", Scheme::Personal),
    ("ES", Scheme::Personal),
    ("FR", Scheme::Personal),
    ("FI", Scheme::Personal),
    ("SE", Scheme::Personal),
    ("BR", Scheme::Personal),
    ("DE", Scheme::Tax),
];

fn scheme_validates(country: &str, value: &str, scheme: Scheme) -> bool {
    if !raw_shape_matches_country(country, value) {
        return false;
    }
    match scheme {
        Scheme::Personal => idsmith::personal_ids()
            .validate(country, value)
            .unwrap_or(false),
        Scheme::Tax => idsmith::tax_ids().validate(country, value),
    }
}

/// idsmith's own per-country `validate()` implementations vary in how
/// strictly they check the raw input shape before checksumming: most
/// (NL, PL, FR, ...) reject anything but a pure digit string of the exact
/// expected length, but BR's CPF validator strips *any* non-digit
/// character from *any* position before checksumming, so a value like
/// "987654321B00" is silently cleaned to "98765432100" and can pass the
/// checksum despite being shape-garbage. Gate that scheme here so a
/// contaminated value never reaches idsmith's lax stripping behavior.
///
/// For BR the gate permits exactly the two conventional CPF shapes: after
/// removing only the standard separators '.' and '-', the remainder must be
/// exactly 11 ASCII digits and nothing else. So both "11144477735" and the
/// canonical formatted "111.444.777-35" pass through to idsmith, while any
/// value carrying a non-separator, non-digit character (e.g. the 'B' in
/// "987654321B00") is rejected before idsmith can strip it away.
fn raw_shape_matches_country(country: &str, value: &str) -> bool {
    match country {
        "BR" => {
            let unseparated: String = value
                .chars()
                .filter(|character| !matches!(character, '.' | '-'))
                .collect();
            unseparated.len() == 11
                && unseparated
                    .chars()
                    .all(|character| character.is_ascii_digit())
        }
        _ => true,
    }
}

fn is_plausible_id_shape(value: &str) -> bool {
    let trimmed = value.trim();
    (6..=20).contains(&trimmed.len())
        && trimmed.chars().any(|character| character.is_ascii_digit())
        && trimmed.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '.' | ' ')
        })
}

pub(in crate::detection) fn national_id_countries(value: &str) -> Vec<&'static str> {
    if !is_plausible_id_shape(value) {
        return Vec::new();
    }
    let trimmed = value.trim();
    ALLOWLIST
        .iter()
        .filter(|(country, scheme)| scheme_validates(country, trimmed, *scheme))
        .map(|(country, _)| *country)
        .collect()
}

pub(in crate::detection) fn is_national_id(value: &str) -> bool {
    !national_id_countries(value).is_empty()
}

#[cfg(test)]
mod probe_tests {
    use super::*;

    // Each vector is a publicly documented test identifier for the scheme.
    // A failing probe means idsmith does not validate that scheme correctly:
    // remove the country from the Task 2 allowlist instead of forcing the test.
    const VALID: &[(&str, &str, Scheme)] = &[
        ("NL", "111222333", Scheme::Personal),        // BSN, 11-proef
        ("BE", "85073003328", Scheme::Personal),      // Rijksregisternummer, mod 97
        ("PL", "44051401359", Scheme::Personal),      // PESEL
        ("IT", "RSSMRA85T10A562S", Scheme::Personal), // Codice fiscale
        ("ES", "12345678Z", Scheme::Personal),        // DNI, mod-23 letter
        ("FR", "255081416802538", Scheme::Personal),  // NIR, mod 97
        ("FI", "131052-308T", Scheme::Personal),      // HETU check char
        ("SE", "811218-9876", Scheme::Personal),      // Personnummer, Luhn
        ("BR", "11144477735", Scheme::Personal),      // CPF
        ("DE", "86095742719", Scheme::Tax),           // Steuer-IdNr (BZSt test number)
    ];

    const INVALID: &[(&str, &str, Scheme)] = &[
        ("NL", "111222334", Scheme::Personal),
        ("BE", "85073003329", Scheme::Personal),
        ("PL", "44051401358", Scheme::Personal),
        ("IT", "RSSMRA85T10A562T", Scheme::Personal),
        ("ES", "12345678A", Scheme::Personal),
        ("FR", "255081416802539", Scheme::Personal),
        ("FI", "131052-308U", Scheme::Personal),
        ("SE", "811218-9875", Scheme::Personal),
        ("BR", "11144477736", Scheme::Personal),
        ("DE", "86095742718", Scheme::Tax),
    ];

    #[test]
    fn idsmith_accepts_documented_valid_ids() {
        for (country, value, scheme) in VALID {
            assert!(
                scheme_validates(country, value, *scheme),
                "expected idsmith to accept {country} {value}"
            );
        }
    }

    #[test]
    fn idsmith_rejects_checksum_near_misses() {
        for (country, value, scheme) in INVALID {
            assert!(
                !scheme_validates(country, value, *scheme),
                "expected idsmith to reject {country} {value}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_bsn_is_national_id() {
        assert!(is_national_id("111222333"));
        assert_eq!(national_id_countries("111222333"), vec!["NL"]);
    }

    #[test]
    fn checksum_near_miss_is_not_national_id() {
        assert!(!is_national_id("111222334"));
    }

    #[test]
    fn short_or_alpha_garbage_is_skipped_cheaply() {
        assert!(!is_national_id("42"));
        assert!(!is_national_id("hello world"));
        assert!(!is_national_id(""));
    }

    #[test]
    fn spanish_dni_with_letter_validates() {
        assert!(is_national_id("12345678Z"));
        assert!(national_id_countries("12345678Z").contains(&"ES"));
    }

    #[test]
    fn german_steuer_id_validates_via_tax_registry() {
        assert!(is_national_id("86095742719"));
        assert!(national_id_countries("86095742719").contains(&"DE"));
    }

    #[test]
    fn formatted_cpf_validates() {
        // The canonical human-readable CPF format uses '.' and '-' separators;
        // the BR shape gate must strip exactly those and nothing more.
        assert!(national_id_countries("111.444.777-35").contains(&"BR"));
        assert!(is_national_id("111.444.777-35"));
    }

    #[test]
    fn letter_contaminated_digits_do_not_pass_via_br_stripping() {
        // idsmith's BR CPF validator strips any non-digit character from any
        // position before checksumming, so "987654321B00" would otherwise be
        // silently cleaned to "98765432100" (a checksum-valid CPF) and match.
        // This is a Dutch BTW-suffix near-miss shape, not a Brazilian CPF.
        assert!(!national_id_countries("987654321B00").contains(&"BR"));
        assert!(!is_national_id("987654321B00"));
    }
}
