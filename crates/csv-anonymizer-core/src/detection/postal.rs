use std::sync::OnceLock;

use regex::Regex;

use super::locale::LocaleContext;

struct PostalFormat {
    country: &'static str,
    pattern: &'static str,
    /// Unambiguous shapes (letters+digits mixes) may match without locale
    /// context; bare-digit shapes (DE, FR, US...) require the country in
    /// context because they collide with SKUs and sequence numbers.
    requires_context: bool,
}

const FORMATS: &[PostalFormat] = &[
    PostalFormat {
        country: "NL",
        pattern: r"^\d{4}\s?[A-Z]{2}$",
        requires_context: false,
    },
    PostalFormat {
        country: "GB",
        pattern: r"^[A-Z]{1,2}\d[A-Z\d]?\s?\d[A-Z]{2}$",
        requires_context: false,
    },
    PostalFormat {
        country: "CA",
        pattern: r"^[A-Z]\d[A-Z]\s?\d[A-Z]\d$",
        requires_context: false,
    },
    PostalFormat {
        country: "PL",
        pattern: r"^\d{2}-\d{3}$",
        requires_context: false,
    },
    PostalFormat {
        country: "PT",
        pattern: r"^\d{4}-\d{3}$",
        requires_context: false,
    },
    PostalFormat {
        country: "BR",
        pattern: r"^\d{5}-\d{3}$",
        requires_context: false,
    },
    PostalFormat {
        country: "JP",
        pattern: r"^\d{3}-\d{4}$",
        requires_context: true,
    },
    PostalFormat {
        country: "US",
        pattern: r"^\d{5}(?:-\d{4})?$",
        requires_context: true,
    },
    PostalFormat {
        country: "DE",
        pattern: r"^\d{5}$",
        requires_context: true,
    },
    PostalFormat {
        country: "FR",
        pattern: r"^\d{5}$",
        requires_context: true,
    },
    PostalFormat {
        country: "IT",
        pattern: r"^\d{5}$",
        requires_context: true,
    },
    PostalFormat {
        country: "ES",
        pattern: r"^\d{5}$",
        requires_context: true,
    },
    PostalFormat {
        country: "SE",
        pattern: r"^\d{3}\s?\d{2}$",
        requires_context: true,
    },
    PostalFormat {
        country: "DK",
        pattern: r"^\d{4}$",
        requires_context: true,
    },
    PostalFormat {
        country: "AT",
        pattern: r"^\d{4}$",
        requires_context: true,
    },
    PostalFormat {
        country: "BE",
        pattern: r"^\d{4}$",
        requires_context: true,
    },
];

fn compiled() -> &'static Vec<(usize, Regex)> {
    static COMPILED: OnceLock<Vec<(usize, Regex)>> = OnceLock::new();
    COMPILED.get_or_init(|| {
        FORMATS
            .iter()
            .enumerate()
            .map(|(index, format)| (index, Regex::new(format.pattern).unwrap()))
            .collect()
    })
}

pub(in crate::detection) fn postal_match_country(
    value: &str,
    locale: &LocaleContext,
) -> Option<&'static str> {
    let trimmed = value.trim().to_ascii_uppercase();
    compiled().iter().find_map(|(index, regex)| {
        let format = &FORMATS[*index];
        let in_context = locale.countries().iter().any(|code| code == format.country);
        if regex.is_match(&trimmed) && (!format.requires_context || in_context) {
            Some(format.country)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nl_postcode_matches_without_context() {
        assert_eq!(
            postal_match_country("1012 AB", &LocaleContext::default()),
            Some("NL")
        );
    }

    #[test]
    fn bare_digit_format_requires_context() {
        assert_eq!(
            postal_match_country("10115", &LocaleContext::default()),
            None
        );
    }
}
