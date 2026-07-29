//! A held-out corpus for measuring detection quality.
//!
//! `multilingual_matrix.rs` is a regression guard: every fixture in it was
//! chosen because the detector handles it, and it asserts precision and recall
//! of exactly 1.0. That makes it useless as a measuring instrument — a suite
//! with no headroom cannot tell you whether a change helped, and in particular
//! cannot answer whether a Local AI assist would add anything.
//!
//! This corpus is the opposite. The cases were written to look like real files
//! the detector was never tuned against: cryptic and abbreviated headers,
//! languages outside the taxonomy, messy value formatting, and benign
//! look-alikes. The detector is *expected to fail some of them*, and the recorded
//! [`baseline`] documents exactly how many. The tests fail on regression, never on
//! imperfection.
//!
//! Working with this file:
//!
//! - Improving the detector makes a score exceed its baseline. The test says so
//!   and tells you to raise the number. Do that in the same commit.
//! - Never "fix" a failure by editing the expectation to match what the
//!   detector currently returns. That converts the instrument back into a
//!   mirror, which is the problem this file exists to solve.
//! - Print the full report any time with:
//!   `cargo test -p csv-anonymizer-core held_out -- --nocapture`

use super::*;
use std::collections::BTreeMap;

/// One held-out column: a header, its sampled values, and what a careful human
/// reviewer would call it.
struct Case {
    /// Why this case is hard, and what it is probing.
    challenge: Challenge,
    header: &'static str,
    values: &'static [&'static str],
    /// The classification a careful reviewer would give this column.
    /// `DataType::String` means "no more specific type applies" — the correct
    /// answer for benign look-alikes that must not be flagged.
    expected: DataType,
    /// A detector that must appear in the column's privacy evidence.
    ///
    /// Needed where the type vocabulary has no name for what the column holds, so
    /// `expected` has to be `String` and the type axis cannot tell success from
    /// "nothing matched". Naming the detector makes the point earned instead of
    /// free. `None` wherever `expected` is discriminating on its own.
    expected_detector: Option<&'static str>,
    /// The lowest privacy risk this column may be assigned. Under-classifying
    /// risk is the dangerous direction: it is what keeps a column from being
    /// auto-selected, so it is scored separately from exact-type accuracy.
    expected_risk_floor: PiiRisk,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Challenge {
    /// Header is abbreviated, machine-generated, or absent.
    CrypticHeader,
    /// Header language is outside the eight-language taxonomy.
    UntaxonomizedLanguage,
    /// Values are a real instance of the type in an awkward format.
    MessyFormatting,
    /// Values resemble a PII type but are not one. Must not be flagged.
    BenignLookAlike,
    /// The column is only partly populated with the type.
    MixedColumn,
}

impl Challenge {
    fn label(self) -> &'static str {
        match self {
            Challenge::CrypticHeader => "cryptic-header",
            Challenge::UntaxonomizedLanguage => "untaxonomized-language",
            Challenge::MessyFormatting => "messy-formatting",
            Challenge::BenignLookAlike => "benign-look-alike",
            Challenge::MixedColumn => "mixed-column",
        }
    }
}

/// Detection quality as measured on this corpus, recorded from an actual run.
///
/// These are floors, not targets. A score below its floor is a regression and
/// fails the build; a score above it means the detector improved and the floor
/// should be raised.
mod baseline {
    /// Cases whose exact `DataType` the detector gets right. Recorded at 18/27.
    /// The nine misses are, by category: person names behind a cryptic or absent
    /// header (name detection is header-gated since the gazetteer was withdrawn,
    /// so it cannot fire), postal codes in an untaxonomized language, an email
    /// wrapped in a display name, European decimal-comma numbers and euro
    /// currency, and IPv6.
    pub(super) const EXACT_TYPE_CORRECT: usize = 18;
    /// Cases assigned at least the privacy risk they warrant. This is the
    /// number that matters for anonymization: it bounds how many columns would
    /// be offered for transformation. Recorded at 23/27 — the four shortfalls
    /// are the three unreachable name columns and IPv6.
    pub(super) const RISK_FLOOR_MET: usize = 23;
    /// Benign look-alikes whose risk landed exactly where it should — neither
    /// under-flagged (a leak) nor over-flagged (lost utility). Recorded at 5/6.
    /// The one shortfall is an over-flag, the safe direction: dotted quads in a
    /// column that is plainly not about networking still become `IpAddress`.
    pub(super) const BENIGN_RISK_EXACT: usize = 5;
}

#[test]
fn held_out_corpus_meets_recorded_baseline() {
    let report = score(cases());
    println!("{}", report.render());

    check(
        &report,
        report.exact_type_correct,
        baseline::EXACT_TYPE_CORRECT,
        "exact-type accuracy",
        "",
    );
    check(
        &report,
        report.risk_floor_met,
        baseline::RISK_FLOOR_MET,
        "risk-floor coverage",
        " This is the privacy-relevant number: a column below its risk floor is a \
         column the app would not offer to anonymize.",
    );
    check(
        &report,
        report.benign_risk_exact,
        baseline::BENIGN_RISK_EXACT,
        "false-positive control",
        "",
    );
}

/// Compares one score against its floor, failing in *either* direction.
///
/// Failing when the score drops is the obvious half. Failing when it rises matters
/// just as much: a floor that silently lags the real number stops being a floor, and
/// the next regression then hides inside the slack. This is the mechanism behind the
/// module docs' instruction to raise the number in the same commit.
fn check(report: &Report, scored: usize, floor: usize, label: &str, note: &str) {
    if scored < floor {
        panic!(
            "{label} regressed: {scored}, baseline {floor}.{note}\n{}",
            report.render()
        );
    }
    assert_eq!(
        scored,
        floor,
        "{label} improved to {scored} from a baseline of {floor}. Raise the baseline \
         in this commit so it keeps measuring — a stale floor hides the next \
         regression.\n{}",
        report.render()
    );
}

/// The corpus only works as an instrument while it still has headroom. If the
/// detector ever scores perfectly here, this file has stopped measuring
/// anything and needs harder cases before it is trusted again.
#[test]
fn held_out_corpus_still_has_headroom() {
    let report = score(cases());

    assert!(
        report.exact_type_correct < report.total,
        "the held-out corpus is now fully solved ({}/{}). It can no longer \
         detect an improvement or a regression in either direction. Add harder \
         cases before relying on it again.\n{}",
        report.exact_type_correct,
        report.total,
        report.render()
    );
}

struct Report {
    total: usize,
    exact_type_correct: usize,
    risk_floor_met: usize,
    benign_total: usize,
    benign_risk_exact: usize,
    by_challenge: BTreeMap<Challenge, (usize, usize)>,
    failures: Vec<String>,
}

impl Report {
    fn render(&self) -> String {
        let mut out = String::from("\nheld-out detection corpus\n");
        out.push_str(&format!(
            "  exact type      {}/{} ({:.0}%)\n",
            self.exact_type_correct,
            self.total,
            percentage(self.exact_type_correct, self.total)
        ));
        out.push_str(&format!(
            "  risk floor met  {}/{} ({:.0}%)\n",
            self.risk_floor_met,
            self.total,
            percentage(self.risk_floor_met, self.total)
        ));
        out.push_str(&format!(
            "  benign risk     {}/{} ({:.0}%)\n",
            self.benign_risk_exact,
            self.benign_total,
            percentage(self.benign_risk_exact, self.benign_total)
        ));
        out.push_str("  by challenge\n");
        for (challenge, (correct, total)) in &self.by_challenge {
            out.push_str(&format!(
                "    {:<24} {correct}/{total}\n",
                challenge.label()
            ));
        }
        if !self.failures.is_empty() {
            out.push_str("  misses\n");
            for failure in &self.failures {
                out.push_str(&format!("    {failure}\n"));
            }
        }
        out
    }
}

fn percentage(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        return 100.0;
    }
    part as f64 * 100.0 / whole as f64
}

fn score(cases: &[Case]) -> Report {
    let mut report = Report {
        total: cases.len(),
        exact_type_correct: 0,
        risk_floor_met: 0,
        benign_total: 0,
        benign_risk_exact: 0,
        by_challenge: BTreeMap::new(),
        failures: Vec::new(),
    };

    for case in cases {
        let detection = detect_column_type_with_name(case.header, &strings(case.values));
        let analysis = analyze_column_privacy(
            case.header,
            0,
            &strings(case.values),
            detection.data_type,
            detection.confidence,
        );
        let risk = max_pii_risk(classify_pii_risk(detection.data_type), analysis.pii_risk);

        // Where the type vocabulary cannot express what the column holds, the type
        // alone is not evidence of anything: `String` is also what a total miss
        // returns. Such a case names the detector that has to have fired, so its
        // point is earned rather than granted by the shape of the enum.
        let detector_fired = case.expected_detector.is_none_or(|detector| {
            analysis
                .evidence
                .iter()
                .any(|summary| summary.detectors.iter().any(|found| found == detector))
        });
        let exact = detection.data_type == case.expected && detector_fired;
        let entry = report.by_challenge.entry(case.challenge).or_insert((0, 0));
        entry.1 += 1;
        if exact {
            report.exact_type_correct += 1;
            entry.0 += 1;
        }

        if risk_rank(risk) >= risk_rank(case.expected_risk_floor) {
            report.risk_floor_met += 1;
        }

        // For benign columns the floor is also a ceiling: over-flagging costs
        // real utility, so the risk has to land exactly where it belongs.
        let benign_over_flagged = case.challenge == Challenge::BenignLookAlike
            && risk_rank(risk) > risk_rank(case.expected_risk_floor);
        if case.challenge == Challenge::BenignLookAlike {
            report.benign_total += 1;
            if risk == case.expected_risk_floor {
                report.benign_risk_exact += 1;
            }
        }

        if !exact || risk_rank(risk) < risk_rank(case.expected_risk_floor) || benign_over_flagged {
            report.failures.push(format!(
                "{:<24} header {:<18} expected {:?}/{:?}, got {:?}/{:?}",
                case.challenge.label(),
                if case.header.is_empty() {
                    "<none>"
                } else {
                    case.header
                },
                case.expected,
                case.expected_risk_floor,
                detection.data_type,
                risk
            ));
        }
    }

    report
}

fn risk_rank(risk: PiiRisk) -> u8 {
    match risk {
        PiiRisk::High => 3,
        PiiRisk::Medium => 2,
        PiiRisk::Low => 1,
    }
}

fn cases() -> &'static [Case] {
    &[
        // --- Cryptic, abbreviated, or absent headers -------------------------
        Case {
            challenge: Challenge::CrypticHeader,
            header: "c_nm",
            values: &[
                "Ada Lovelace",
                "Grace Hopper",
                "Alan Turing",
                "Edsger Dijkstra",
            ],
            expected: DataType::FullName,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::CrypticHeader,
            header: "eml_addr",
            values: &["ada@example.com", "grace@example.org", "alan@example.net"],
            expected: DataType::Email,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::CrypticHeader,
            header: "col_7",
            values: &["+1 415 555 0100", "+1 212 555 0101", "+1 312 555 0102"],
            expected: DataType::Phone,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::CrypticHeader,
            header: "",
            values: &[
                "Marie Dubois",
                "Luc Martin",
                "Jean Bernard",
                "Sophie Moreau",
            ],
            expected: DataType::FullName,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::CrypticHeader,
            header: "f3",
            values: &[
                "Hauptstrasse 12",
                "Marktplatz 5",
                "Bahnhofstrasse 88",
                "Lindenweg 3",
            ],
            expected: DataType::Address,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::CrypticHeader,
            header: "dob",
            values: &["1980-01-02", "1991-03-04", "1975-11-30", "1966-06-21"],
            expected: DataType::Timestamp,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Medium,
        },
        // --- Languages outside the eight-language taxonomy ------------------
        Case {
            challenge: Challenge::UntaxonomizedLanguage,
            header: "imię_i_nazwisko",
            values: &["Jan Kowalski", "Anna Nowak", "Piotr Wiśniewski"],
            expected: DataType::FullName,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::UntaxonomizedLanguage,
            header: "telefonnummer",
            values: &["+46 70 123 45 67", "+46 73 234 56 78", "+46 76 345 67 89"],
            expected: DataType::Phone,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::UntaxonomizedLanguage,
            header: "adres",
            values: &["ul. Marszałkowska 10", "ul. Długa 22", "ul. Krótka 4"],
            expected: DataType::Address,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::UntaxonomizedLanguage,
            header: "posta_kodu",
            values: &["34710", "06510", "35220", "01120"],
            expected: DataType::PostalCode,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Medium,
        },
        Case {
            challenge: Challenge::UntaxonomizedLanguage,
            header: "e-posta",
            values: &["ayse@example.com.tr", "mehmet@example.tr"],
            expected: DataType::Email,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        // --- Real values in awkward formats ---------------------------------
        Case {
            challenge: Challenge::MessyFormatting,
            header: "contact",
            values: &[
                "Ada Lovelace <ada@example.com>",
                "Grace Hopper <grace@example.org>",
            ],
            expected: DataType::Email,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::MessyFormatting,
            header: "phone",
            values: &[
                "+1 415 555 0100 x22",
                "+1 212 555 0101 ext. 8",
                "+1 312 555 0102",
            ],
            expected: DataType::Phone,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::MessyFormatting,
            header: "bedrag",
            values: &["€ 1.234,56", "€ 987,00", "€ 12.000,10"],
            expected: DataType::Currency,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::MessyFormatting,
            header: "omzet",
            values: &["1.234,56", "987,00", "12.000,10", "45,99"],
            expected: DataType::NumericValue,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::MessyFormatting,
            header: "server_addr",
            values: &["2001:db8::1", "2001:db8:85a3::8a2e:370:7334", "fe80::1"],
            expected: DataType::IpAddress,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Medium,
        },
        // There is no `DataType::Iban`, so `String` is the most specific answer
        // available — and `String` is also what a total miss returns, which would
        // score this case correct for recognizing nothing. Hence the named
        // detector: the point is only awarded if the checksum validator actually
        // fired on these space-separated IBANs.
        Case {
            challenge: Challenge::MessyFormatting,
            header: "iban",
            values: &[
                "NL91 ABNA 0417 1643 00",
                "DE89 3704 0044 0532 0130 00",
                "FR14 2004 1010 0505 0001 3M02 606",
            ],
            expected: DataType::String,
            expected_detector: Some("validator:iban"),
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::MessyFormatting,
            header: "email",
            values: &["ADA@EXAMPLE.COM", " grace@example.org ", "Alan@Example.Net"],
            expected: DataType::Email,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        // --- Benign look-alikes that must not be flagged --------------------
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "sku",
            values: &["SKU-00012", "SKU-00013", "SKU-00014", "SKU-00015"],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "app_version",
            values: &["1.2.3", "1.2.4", "10.0.1", "2.11.0"],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "checksum",
            values: &[
                "d41d8cd98f00b204e9800998ecf8427e",
                "5d41402abc4b2a76b9719d911017c592",
            ],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "product_name",
            values: &["Blue Widget", "Red Gadget", "Green Sprocket", "Grey Flange"],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        // Dotted quads in a column that is plainly not about networking. Every
        // value is valid IPv4 syntax, so this probes whether a non-network
        // header keeps a shape match from escalating the column's risk.
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "measurement",
            values: &["12.34.56.78", "98.76.54.32", "11.22.33.44"],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Low,
        },
        // Sequence numbers are genuinely `NumericId`, and the app treats that as
        // Medium risk by design — a surrogate key can still be a re-identifier.
        // Scored as an exact-risk case, so escalating it further would fail.
        Case {
            challenge: Challenge::BenignLookAlike,
            header: "seq",
            values: &["1000001", "1000002", "1000003", "1000004"],
            expected: DataType::NumericId,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Medium,
        },
        // --- Partly-populated columns ---------------------------------------
        Case {
            challenge: Challenge::MixedColumn,
            header: "notes",
            values: &[
                "call back",
                "ada@example.com",
                "no answer",
                "grace@example.org",
                "left voicemail",
            ],
            expected: DataType::String,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::MixedColumn,
            header: "identifier",
            values: &[
                "ada@example.com",
                "grace@example.org",
                "alan@example.net",
                "unknown",
                "n/a",
            ],
            expected: DataType::Email,
            expected_detector: None,
            expected_risk_floor: PiiRisk::High,
        },
        Case {
            challenge: Challenge::MixedColumn,
            header: "reference",
            values: &[
                "550e8400-e29b-41d4-a716-446655440000",
                "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
                "pending",
                "7c9e6679-7425-40de-944b-e07fc1f90ae7",
            ],
            expected: DataType::Uuid,
            expected_detector: None,
            expected_risk_floor: PiiRisk::Medium,
        },
    ]
}
