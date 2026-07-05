//! National-ID validation via the idsmith registries.
//! Only checksum-backed schemes are allowlisted; format-only kinds
//! (passports, driver's licenses) match arbitrary IDs too often to vote.

#[cfg(test)]
mod probe_tests {
    // Each vector is a publicly documented test identifier for the scheme.
    // A failing probe means idsmith does not validate that scheme correctly:
    // remove the country from the Task 2 allowlist instead of forcing the test.
    const VALID: &[(&str, &str, Registry)] = &[
        ("NL", "111222333", Registry::Personal),   // BSN, 11-proef
        ("BE", "85073003328", Registry::Personal), // Rijksregisternummer, mod 97
        ("PL", "44051401359", Registry::Personal), // PESEL
        ("IT", "RSSMRA85T10A562S", Registry::Personal), // Codice fiscale
        ("ES", "12345678Z", Registry::Personal),   // DNI, mod-23 letter
        ("FR", "255081416802538", Registry::Personal), // NIR, mod 97
        ("FI", "131052-308T", Registry::Personal), // HETU check char
        ("SE", "811218-9876", Registry::Personal), // Personnummer, Luhn
        ("BR", "11144477735", Registry::Personal), // CPF
        ("DE", "86095742719", Registry::Tax),      // Steuer-IdNr (BZSt test number)
    ];

    const INVALID: &[(&str, &str, Registry)] = &[
        ("NL", "111222334", Registry::Personal),
        ("PL", "44051401358", Registry::Personal),
        ("ES", "12345678A", Registry::Personal),
        ("SE", "811218-9875", Registry::Personal),
        ("BR", "11144477736", Registry::Personal),
        ("DE", "86095742718", Registry::Tax),
    ];

    #[derive(Clone, Copy)]
    enum Registry {
        Personal,
        Tax,
    }

    fn validates(country: &str, value: &str, registry: Registry) -> bool {
        match registry {
            Registry::Personal => idsmith::personal_ids()
                .validate(country, value)
                .unwrap_or(false),
            Registry::Tax => idsmith::tax_ids().validate(country, value),
        }
    }

    #[test]
    fn idsmith_accepts_documented_valid_ids() {
        for (country, value, registry) in VALID {
            assert!(
                validates(country, value, *registry),
                "expected idsmith to accept {country} {value}"
            );
        }
    }

    #[test]
    fn idsmith_rejects_checksum_near_misses() {
        for (country, value, registry) in INVALID {
            assert!(
                !validates(country, value, *registry),
                "expected idsmith to reject {country} {value}"
            );
        }
    }
}
