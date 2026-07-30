//! How many released rows stand alone once every column is read together.
//!
//! Every other privacy figure in this crate is per column.
//! [`crate::types::ColumnValueDistribution`] says what one column's pseudonyms leak about
//! that column; `report_identifier_class_for_column` counts identifiers one at a time;
//! `preview_warning_for_column` warns one column at a time. Nothing looks at two columns
//! together, so the release report can state that no high or medium risk column was left
//! unselected about a file in which postcode, birth date and job title jointly single out
//! a third of the rows.
//!
//! This module measures that: over one streaming pass, how many released rows are alone
//! in their equivalence class on the columns an outsider could actually match against
//! data they already hold.

use crate::detection::is_empty_value;
use crate::strategies::is_phone_shaped;
use crate::types::{
    AnonymizationStrategy, ColumnMetadata, DataType, DropColumnEffect, MatchedColumn, MatchedPart,
    RowUniquenessSummary,
};
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Classes either map may hold before measurement stops.
///
/// Worst case is one entry per row in each of the two maps. A `HashMap<u128, u32>` entry
/// is 32 bytes of payload once the `u128`'s 16-byte alignment is paid, plus hashbrown's
/// control byte, over a 0.875 load factor: roughly 38 bytes. Two maps at this ceiling is
/// therefore about 150 MB, which sits an order of magnitude below the mapping ceiling's
/// 5.1 GB and leaves the uniqueness check a junior claim on memory — as it should be,
/// since it is a report figure and the mapping is the output's correctness.
///
/// Derived from the type layout rather than measured on a running process, unlike
/// [`crate::strategies::TransformState::APPROXIMATE_BYTES_PER_MAPPING_ENTRY`], which was
/// read off `VmHWM`. That is acceptable here and would not have been there, because of
/// the difference in what being wrong costs: an under-estimate of the mapping's memory
/// gets the process OOM-killed, while an under-estimate here only stops a measurement
/// early and says so.
///
/// **What the whole measure costs, measured.** On 200,000 rows of seven columns, release
/// build: about 390 ms with [`RowUniquenessTracker::record_row`] stubbed out, and about
/// 730 ms with it live — so the measure roughly doubles the transform. Peak RSS 61 MB, an
/// order of magnitude under this ceiling's own bound. The cost splits roughly evenly across
/// the three histograms: dropping `all_column_classes` or the attribution each recovered
/// about 140 ms, so there is no single component worth cutting, and cutting either one
/// costs a reported figure.
///
/// Recorded because the obvious response to the number is an opt-out, and that is worse
/// than the cost. A switch that turns a privacy measure off is one somebody enables for a
/// quick run and never disables, and the report would then be missing its strongest finding
/// with nothing saying so — where every ceiling here suppresses a figure *and* sets a flag
/// that makes the suppression visible.
const CLASS_CEILING: usize = 2_000_000;

/// Columns the leave-one-out attribution will track, above which it does not run.
///
/// A per-row cost, not a memory one — memory is bounded separately and much more tightly by
/// [`ATTRIBUTION_CLASS_CEILING`]. Each tracked column costs one hash-map probe per row, so
/// this bounds the attribution at roughly this multiple of the joint measure's own per-row
/// work. It can be generous precisely because [`RowUniquenessTracker::record_row`] derives
/// all of the leave-one-out keys from one pass over the projections rather than re-hashing
/// the row once per column; the naive shape is quadratic in the column count and would force
/// a cap around a third of this.
///
/// A file wider than this gets no attribution and is told so, which is the same
/// incomplete-measurement idiom the ceilings above use: the alternative is a report that
/// quietly says nothing about the widest files, which are the ones most likely to need it.
const ATTRIBUTION_COLUMN_CAP: usize = 24;

/// Classes the leave-one-out histograms may hold *between them* before attribution stops.
///
/// Shared rather than per column, because the per-column bound is the one that does not
/// compose: twenty-four maps each allowed [`CLASS_CEILING`] entries is 48 million entries,
/// about 1.8 GB by the 38-bytes-per-entry arithmetic in that constant's doc, which would make
/// an actionable footnote by far the largest allocation in the process.
///
/// At this shared figure the attribution holds 4 million entries. That is roughly 150 MB at
/// the 0.875 load factor the sibling constant assumes, but twenty-four independently-grown
/// hashbrown tables cannot all sit at that load: just past a doubling each is nearer 0.44,
/// which is about 300 MB. So the honest bound is that the attribution can cost around twice
/// what the two histograms above cost together, not the same — still an order of magnitude
/// below the mapping's 5.1 GB, which is the claim that has to hold.
///
/// **When this bites is not what it looks like.** Every counted column gets a histogram,
/// including the ones [`RowUniquenessTracker::is_matched`] will never report, and a column
/// whose projection is *constant* fills the fastest of all: its key is the row total minus a
/// fixed component, which is a bijection, so its map holds one entry per distinct row — the
/// most any map here can hold. The crossover is therefore near this ceiling divided by the
/// column count, not by anything about the file's width: about 167,000 distinct rows at the
/// column cap, and about 667,000 for an ordinary six-column file. Both are below the joint
/// measure's own 2,000,000, so on a mid-sized file the attribution stops first, by design and
/// not by accident.
///
/// When the ceiling bites, the report says the attribution was not measured and every figure
/// it does state stays exact — so this bound costs advice rather than findings.
///
/// A cheaper design exists and is deliberately not taken: buffering prefix totals would let a
/// still-constant column's map be rebuilt lazily, without touching the joint histogram. It is
/// declined because a short rehydrated map lowers `unique_rows_without`, and understating that
/// figure is the one direction this module may not be wrong in.
const ATTRIBUTION_CLASS_CEILING: usize = 4_000_000;

/// Stands in for a digit in [`LinkableProjection::PhoneDialLayout`].
///
/// A character `is_phone_shaped` rejects, which is the entire requirement and is why it is a
/// named constant. `'#'` must not be used: it is accepted in phone values as the DTMF hash
/// key, so `"0612345678#"` and `"06123456785"` would both collapse to `"###########"` and two
/// separately-filterable dial layouts would merge into one class. Merging makes classes look
/// larger and the file look safer, which the doc on [`hash_fields`] calls the one direction a
/// privacy figure must not be wrong in — reintroduced one level above the hash it was
/// engineered out of.
const DIGIT_PLACEHOLDER: char = '\u{0}';

/// The part of a released value an outsider holding the original could reproduce.
///
/// The subset rule turns on one question: **given the original value, can someone compute
/// or recognise what we released?** Not "did anything survive the transform" — an opaque
/// token on a primary key makes every row unique and helps an attacker not at all, because
/// they cannot derive `a7f3c2` from anything they hold. Counting such a column would make
/// the metric fire on every file with a primary key, and a measure that always fires
/// carries no information.
///
/// Some transforms are partially reproducible, and those are projected rather than
/// included whole. A pseudonymized email keeps its domain and replaces its local part: an
/// outsider can reproduce `@corp.com` but not `user417`. Hashing the whole cell would make
/// every row look unique on a column that in truth only sorts rows by employer, which
/// over-states risk in exactly the direction that trains people to ignore the number.
///
/// The line is *exact* reproducibility, not usefulness. The format-only survivors below —
/// a digit count, a count of name parts — stay in even though each has few distinct values,
/// because this measure's entire premise is that weak signals combine into a strong one, and
/// excluding one on a guess about its size predicts an effect instead of measuring it. Where
/// that makes a finding read badly, the repair belongs in [`Self::matched_part`] and not in
/// the count. What stays excluded is excluded because it genuinely cannot be reproduced: a
/// generic-string pseudonym keeps a length within about 20%, and nobody can filter on "about".
///
/// Exactly one projection breaks that line, on purpose, and it is the reason this comment
/// has a sequel. See [`Self::TimestampDecadeAndTimeOfDay`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkableProjection {
    /// The released cell is the original, or is derived from it by a rule anyone can
    /// apply. Hash it whole.
    WholeValue,
    /// The whole released cell again, but a masked one: `*** ** *****` rather than a value.
    ///
    /// Hashed identically to [`Self::WholeValue`] and *reported* differently, which is the
    /// only reason it is a separate variant. A masked cell is exactly reproducible from the
    /// original, so it belongs in the measure — but naming the column bare would claim the
    /// released file contains the names, and it contains their skeletons. Folding it into
    /// `WholeValue` produced "Every released row shares full_name, city with at least 7
    /// other(s)" about a file where `full_name` was masked on every row.
    MaskedSkeleton,
    /// Which cells are blank, and with which blank token.
    ///
    /// `transform_value_with_state` returns an empty-ish cell *verbatim* before any strategy
    /// runs, so a redacted, labelled or tokenized column still publishes its missingness
    /// pattern — and whether a given row wrote `""`, `NULL` or `null`. Someone holding the
    /// original record knows which of its fields were blank, so this is reproducible under
    /// the same rule as everything else here, and it is a well-worn quasi-identifier: which
    /// questions a person did not answer.
    ///
    /// Every column that survives no other way carries this one, which is why nothing is
    /// exempt from the measure any more. On a column with no blanks it is constant and the
    /// column is dropped by [`ProjectionWitness::is_varied`], so the common case costs a
    /// reader nothing.
    BlankCellPattern,
    /// Everything from the last `@`. `transform_email` keeps the domain verbatim.
    EmailDomain,
    /// The decade of the released date, plus everything after the date.
    ///
    /// The one approximate projection here, and the only one that had to be. Exact
    /// reproducibility rules a pseudonymized date out altogether: `transform_timestamp`
    /// draws an independent offset of up to ±365 days per source value, so nobody holding
    /// the original can say which date we published. That answer is unusable. A date of
    /// birth is the textbook quasi-identifier, and a rule that scores it at zero zeroes
    /// out the strongest signal this whole measure exists to catch.
    ///
    /// The released *year* is the obvious repair and is worse than the disease. Two
    /// candidate years and roughly one row per birth date makes nearly every row unique,
    /// on nearly every file with a date column, while the attacker's true candidate set is
    /// every row inside their ±1-year window — frequently dozens. A measure that reports
    /// `k = 1` where `k` is really 30 is not a measurement, it is an alarm wired to the
    /// front door, and the first thing anyone does with one of those is stop reading it.
    ///
    /// A decade is approximate in the same way the attack is approximate. A 1984 birth
    /// date is released somewhere in 1983–1985, so filtering on "the 1980s" is right
    /// unless the original sat within a year of the boundary. It splits classes it should
    /// not across those boundaries, and it merges rows the attacker would also have
    /// trouble separating; both errors are the size of the attacker's own uncertainty
    /// rather than an order of magnitude past it. Month and day are dropped because
    /// nothing about them survives the shift.
    ///
    /// The suffix is exact, and in practice it is the more dangerous half: a source
    /// carrying sub-second precision comes through untouched, which is close to unique per
    /// event.
    TimestampDecadeAndTimeOfDay,
    /// Every non-digit kept, every digit replaced. `transform_phone_candidate` redraws
    /// digits only, so the separator layout, the punctuation and the digit count all come
    /// through exactly as the original had them.
    PhoneDialLayout,
    /// Digit count and leading-zero count. `transform_numeric_id_candidate` preserves the
    /// width always, and the leading-zero count for every value that has a significant
    /// digit, so `00042` stays a five-wide value with three leading zeros. An all-zeros id
    /// is the one exception — it has nothing significant to preserve, so the replacement
    /// may lead with any digit and this projection reads a zero count where the original
    /// had five. That misplaces one value out of a value domain nobody uses as an
    /// identifier, and it is recorded here rather than guarded because the guard would need
    /// the original, which by design this function does not have.
    NumericIdWidth,
    /// Sign, integer width and decimal places, each preserved by
    /// `transform_numeric_value_candidate`.
    NumericValueWidth,
    /// The number of whitespace-separated parts. `transform_full_name` replaces each token
    /// from a pool and keeps their count, so a three-part name stays three parts.
    NameTokenCount,
    /// Whether the value is upper case. `transform_uuid` redraws the UUID and copies the
    /// original's case. Two values wide and it narrows almost nobody down, and it is
    /// counted rather than judged for that reason: weak signals are measured here, not
    /// predicted away.
    ///
    /// A redrawn UUID that happens to contain no letters at all is upper case and lower
    /// case at once — 32 hex draws missing `a`–`f`, about one value in seven million — so it
    /// gets a third class of its own rather than being folded into either. See `apply`.
    UuidLetterCase,
}

impl LinkableProjection {
    /// The projection to apply to `column`.
    ///
    /// Total, and it has to be: no column leaks nothing, because a blank cell is written
    /// through verbatim whatever the strategy says. A column with no other surviving signal
    /// gets [`Self::BlankCellPattern`], which costs a reader nothing on a column with no
    /// blanks because a constant projection is dropped before the column is named.
    ///
    /// No wildcard arm on either match. A strategy or data type added later has to be
    /// classified here rather than defaulting into the quietest answer, because the quietest
    /// answer is the one that under-states risk.
    fn for_column(column: &ColumnMetadata) -> Self {
        // An unselected column is written through untouched by
        // `transform_row_with_state`, whatever `strategy` says it would have done. What
        // the file contains is the original, so read it as pass-through and not as the
        // strategy the user picked but never applied.
        if !column.is_selected {
            return Self::WholeValue;
        }

        match column.strategy {
            AnonymizationStrategy::PassThrough => Self::WholeValue,
            // `***  ** *****` is reproducible from "Jan de Vries" by anyone, which is what
            // makes masking linkable however unreadable it looks — and is also why it is not
            // `WholeValue`: what is reproducible is the skeleton, not the name.
            AnonymizationStrategy::Mask => Self::MaskedSkeleton,
            // An opaque token, an internal ordinal, and a constant. None can be derived from
            // the original: an attacker who knows the subject cannot pick their row out by
            // any of them, however distinct the released values are. What they *can* pick it
            // out by is which of its cells are blank, which no strategy touches.
            AnonymizationStrategy::Tokenize
            | AnonymizationStrategy::Label
            | AnonymizationStrategy::Redact => Self::BlankCellPattern,
            // A value the model actually produced is invented and reproducible from
            // nothing. The interesting case is the other one: when `crate::smart`'s leak
            // guard refuses a replacement, `strategies::transform_value_with_state` sends
            // the value down the *pseudonymizing* transformers — the very ones whose
            // reproducible leftovers every projection above exists to catch. So the class
            // here has to be the fallback's class; anything quieter under-states a whole
            // column's risk.
            //
            // Nor is that path rare. With no provider configured every value in the column
            // falls back, and once a column passes `SMART_REPLACEMENT_VALUE_CAP_PER_COLUMN`
            // distinct values every later value falls back too. The measure cannot tell
            // per row which happened, so it reports the upper bound, which is the only
            // direction this module is allowed to be wrong in.
            AnonymizationStrategy::LocalAi => {
                Self::for_local_ai_fallback_type(column.detected_type)
            }
            AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {
                Self::for_pseudonymized_type(column.detected_type)
            }
        }
    }

    /// The projection for a column whose Local AI replacement was refused.
    ///
    /// The fallback lands on the same transformers as `Pseudonymize` with one difference,
    /// and the difference is the reason this is a separate function rather than a call to
    /// [`Self::for_pseudonymized_type`]. The `uses_default_pass_through` types are *not*
    /// passed through on this path: `transform_value_with_state` skips that gate precisely
    /// so a refused value is never published verbatim, and what those five types get
    /// instead is the generic-string transformer. A generic-string pseudonym keeps only an
    /// approximate length, so on this path they are unlinkable where under `Pseudonymize`
    /// they are the original.
    ///
    /// The predicate is called rather than re-listed so that this classification and the
    /// gate it is describing cannot drift apart.
    fn for_local_ai_fallback_type(data_type: DataType) -> Self {
        if data_type.uses_default_pass_through() {
            return Self::BlankCellPattern;
        }
        Self::for_pseudonymized_type(data_type)
    }

    fn for_pseudonymized_type(data_type: DataType) -> Self {
        match data_type {
            DataType::Email => Self::EmailDomain,
            DataType::Timestamp => Self::TimestampDecadeAndTimeOfDay,
            // Format survives these exactly, so each is counted through the property it
            // keeps. Individually none of them singles anyone out; a joint measure is
            // precisely the place where that stops being the right question.
            DataType::Phone => Self::PhoneDialLayout,
            DataType::NumericId => Self::NumericIdWidth,
            DataType::NumericValue => Self::NumericValueWidth,
            DataType::FirstName | DataType::LastName | DataType::FullName => Self::NameTokenCount,
            DataType::Uuid => Self::UuidLetterCase,
            // Everything here takes the generic-string path, which draws a replacement of
            // 80–120% of the original's length. An approximate length is not reproducible:
            // someone holding the original cannot say which released cells it could have
            // become, so there is no filter to apply and nothing to count. This is the
            // only exclusion left on the pseudonymizing side, and it rests on the transform
            // rather than on a guess about how much the signal is worth.
            DataType::Address
            | DataType::PostalCode
            | DataType::IpAddress
            | DataType::Url
            | DataType::MacAddress
            | DataType::TaxId
            | DataType::String
            | DataType::Unknown => Self::BlankCellPattern,
            // Returned unchanged under these two strategies, so the released cell is the
            // original. These are exactly the types `uses_default_pass_through` names;
            // they are listed rather than delegated so this match stays exhaustive and a
            // type added to the enum has to be classified here.
            DataType::Enum
            | DataType::CountryCode
            | DataType::Boolean
            | DataType::Currency
            | DataType::Percentage => Self::WholeValue,
        }
    }

    /// The reproducible part of one released cell, or `None` where there is none.
    ///
    /// `None` is the shape-fallback answer: a cell that did not match its column's detected
    /// shape is replaced by a generic pseudonym, so a timestamp column can hold a value with
    /// no time in it. It hashes as the empty string, which is the correct reading — there is
    /// nothing there to link on — so this distinction changes no count.
    ///
    /// It exists because `Some("")` and `None` are different facts and the report needs to
    /// tell them apart. `BlankCellPattern` returns `Some("")` for a cell with something in
    /// it: "not blank" is the projection working, not failing, and a column of entirely
    /// non-blank cells is matched on all of them. `WholeValue` returns `Some("")` for a
    /// genuinely empty cell, for the same reason. Returning a bare `Cow` made those
    /// indistinguishable from a timestamp that could not be parsed, and the first thing that
    /// tried to use the difference — the caveat naming columns only some rows carry — got
    /// every blank-pattern column in the suite wrong.
    fn apply(self, released: &str) -> Option<Cow<'_, str>> {
        match self {
            // Hashed identically; they differ only in how the finding is allowed to word
            // them. See `MaskedSkeleton`.
            Self::WholeValue | Self::MaskedSkeleton => Some(Cow::Borrowed(released)),
            // The blank token itself, so that a row writing `NULL` and a row writing `""`
            // land in different classes — both are reproducible by someone holding the
            // original record. A cell with anything in it contributes nothing here, which is
            // what keeps this projection silent on a column with no blanks.
            Self::BlankCellPattern => Some(if is_empty_value(released.trim()) {
                Cow::Owned(format!("blank:{released}"))
            } else {
                // `Some("")`, emphatically not `None`. "This cell has something in it" is
                // this projection succeeding: the pattern of which cells are blank is
                // reproducible from the original either way, so every row is matched on it.
                Cow::Borrowed("")
            }),
            Self::EmailDomain => released
                .rfind('@')
                .map(|index| Cow::Borrowed(&released[index..])),
            Self::TimestampDecadeAndTimeOfDay => decade_and_suffix(released)
                .map(|(decade, suffix)| Cow::Owned(format!("{decade}|{suffix}"))),
            // Digits carry none of the original; everything between them carries all of
            // its layout. Collapsing the digits to one character keeps the count as well,
            // since the placeholders are positional.
            //
            // Gated on the same predicate that decided the transform, because a value that
            // did not look like a phone number was replaced by a generic pseudonym and
            // digit-masking one of those publishes a class key made of leftover random
            // letters. That is how a three-row file came out jointly unique on a column
            // where nothing reproducible had survived at all — a false alarm, which is the
            // failure that teaches people to ignore the true ones.
            Self::PhoneDialLayout => {
                if is_phone_shaped(released) {
                    Some(Cow::Owned(
                        released
                            .chars()
                            .map(|character| {
                                if character.is_ascii_digit() {
                                    DIGIT_PLACEHOLDER
                                } else {
                                    character
                                }
                            })
                            .collect(),
                    ))
                } else {
                    None
                }
            }
            Self::NumericIdWidth => {
                let leading_zeros = released.chars().take_while(|c| *c == '0').count();
                // `len`, not `chars().count()`, because `len` is the width
                // `transform_numeric_id_candidate` reads off the source and reproduces.
                // The two agree on every value it emits — those are ASCII digits — and
                // using the transformer's own measure is what keeps them agreeing.
                Some(Cow::Owned(format!("{}:{leading_zeros}", released.len())))
            }
            Self::NumericValueWidth => {
                let (sign, unsigned) = match released.as_bytes().first() {
                    Some(b'+') | Some(b'-') => released.split_at(1),
                    _ => ("", released),
                };
                let (integer, fraction) = match unsigned.split_once('.') {
                    Some((integer, fraction)) => (integer, Some(fraction.len())),
                    None => (unsigned, None),
                };
                // The integer part's leading zeros, which `generate_numeric_component`
                // reproduces byte for byte — and an all-zero component it returns *verbatim*.
                // Reading only the widths threw that away, so `0.5` and `4.2` shared a class
                // while an outsider holding either one could tell the two apart on the first
                // character. Merging classes makes the file look safer, which is the one
                // direction this module may not be wrong in, and the sibling `NumericIdWidth`
                // had read its leading zeros all along.
                let leading_zeros = integer.chars().take_while(|c| *c == '0').count();
                let integer = format!("{}:{leading_zeros}", integer.len());
                Some(Cow::Owned(match fraction {
                    Some(places) => format!("{sign}{integer}.{places}"),
                    None => format!("{sign}{integer}"),
                }))
            }
            Self::NameTokenCount => {
                Some(Cow::Owned(released.split_whitespace().count().to_string()))
            }
            // Three classes, not two. A redrawn UUID that happens to contain no letters at
            // all is upper case and lower case at once, so folding it into either would merge
            // it with values whose original case is known — under-stating risk about one
            // value in seven million. Its own class over-states instead, which is the
            // direction this module is allowed to be wrong in, and it is also the honest
            // reading: an outsider filtering on case cannot place such a value either.
            Self::UuidLetterCase => Some(Cow::Borrowed(
                if !released.chars().any(char::is_alphabetic) {
                    "no-letters"
                } else if released == released.to_uppercase() {
                    "upper"
                } else {
                    "other"
                },
            )),
        }
    }

    /// How the report is allowed to describe this column.
    ///
    /// The report cannot be trusted to work this out from a column index, and when it tried
    /// it got it wrong: a projection that keeps *part* of a value was named as though it
    /// kept all of it, so a file whose rows shared only an email domain and a birth decade
    /// was reported as sharing "their combination of birth_date, email". Every projection
    /// therefore states its own reporting class here, next to the code that defines what it
    /// extracts, and `release_report` only formats what it is told.
    ///
    /// The five format-only projections collapse to one variant because the wording for
    /// them is identical — "the surviving format of X" — and because a reader told their
    /// rows are unique "on postal_code, customer_id" would remove the customer id and
    /// change nothing, whichever format property it was that survived.
    fn matched_part(self) -> MatchedPart {
        match self {
            Self::WholeValue => MatchedPart::WholeValue,
            Self::EmailDomain => MatchedPart::EmailDomain,
            Self::TimestampDecadeAndTimeOfDay => MatchedPart::DateDecadeAndTime,
            Self::BlankCellPattern => MatchedPart::BlankPattern,
            // A mask publishes a length, a word count and per-word letter counts. That is a
            // surviving format in exactly the sense the other five are, and saying so is the
            // whole reason it is not hashed as a whole value.
            Self::MaskedSkeleton
            | Self::PhoneDialLayout
            | Self::NumericIdWidth
            | Self::NumericValueWidth
            | Self::NameTokenCount
            | Self::UuidLetterCase => MatchedPart::SurvivingFormat,
        }
    }
}

/// The decade of a released date, and everything after the date, or `None` when the value
/// does not start with one.
///
/// `None` is the answer for a shape-fallback value: a cell that missed its column's detected
/// shape was replaced by a generic pseudonym, so a timestamp column can hold a value with no
/// date in it, and slicing one at a fixed offset would hash arbitrary characters as though
/// they were a time.
///
/// The year is matched at *four or more* digits rather than exactly four. `chrono` writes a
/// year outside `0..=9999` in expanded form — `+10000-01-01` — which a fixed ten-character
/// prefix rejected, so a source date near the end of the supported range plus a positive
/// shift projected to nothing and merged with every other unparseable row. Merging
/// under-states risk, so the narrow parse failed in the one direction this module may not
/// fail in, however rare the input.
///
/// Every offset below is derived from a run of ASCII bytes, so the string slices cannot land
/// inside a multi-byte character.
fn decade_and_suffix(released: &str) -> Option<(&str, &str)> {
    let bytes = released.as_bytes();
    let year_start = usize::from(matches!(bytes.first(), Some(b'+') | Some(b'-')));
    let year_digits = bytes
        .get(year_start..)?
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if year_digits < 4 {
        return None;
    }

    // `-MM-DD` has to follow, and is then discarded: nothing about a month or a day survives
    // a shift of up to a year in either direction.
    let year_end = year_start + year_digits;
    let month_day = bytes.get(year_end..year_end + 6)?;
    let well_formed = month_day[0] == b'-'
        && month_day[3] == b'-'
        && month_day[1..3].iter().all(u8::is_ascii_digit)
        && month_day[4..6].iter().all(u8::is_ascii_digit);
    if !well_formed {
        return None;
    }

    // All the year but its last digit, sign included so that a negative year and a positive
    // one cannot share a decade.
    Some((&released[..year_end - 1], &released[year_end + 6..]))
}

/// One column the measure reads, resolved once when the first row arrives.
#[derive(Debug, Clone)]
struct CountedColumn {
    /// Position in the released row, which is what [`RowUniquenessTracker::record_row`]
    /// indexes with.
    position: usize,
    /// `ColumnMetadata::index`, which is what the report names columns by. The two differ
    /// on any input whose metadata is not a dense prefix of the row.
    column_index: usize,
    projection: LinkableProjection,
    /// What this column's projection has done across the rows seen so far.
    witness: ProjectionWitness,
    /// How many rows this column's projection actually applied to.
    ///
    /// `yielded` answers "ever", which is what decides whether the column is named at all.
    /// This answers "how often", which decides how the naming is allowed to be worded. The
    /// two come apart on the shape-fallback paths: [`LinkableProjection::apply`] returns an
    /// empty slice for a cell that did not match its column's detected shape, so a
    /// `Timestamp` column where one value parses and ninety-nine were pseudonymized
    /// generically has `yielded == true` and this at 1. The report then said the rows
    /// "share the decade and time of birth_date" of ninety-nine rows carrying no decade at
    /// all.
    ///
    /// The *counts* were never wrong — an outsider holding the original cannot use that
    /// column on those rows either, so hashing them as sharing nothing there is the honest
    /// thing — but the sentence explaining the counts was, and the sentence is what a reader
    /// acts on.
    rows_yielded: usize,
}

/// What a column's projection has done so far, as one value rather than several flags.
///
/// The two questions the report asks — "did this column ever yield anything?" and "did it
/// ever *change*?" — are not independent, and holding them as two `bool`s let them be set
/// apart. Two distinct projections cannot both be empty, so `Varied` always implies
/// yielded; here that holds by construction rather than by discipline.
///
/// Both questions are load-bearing, and each was learned from a wrong report:
///
/// - Without the yielded question, a projection silent on every row was still named and
///   still counted towards a `Verified` claim — a `Timestamp` column whose values hold no
///   ISO date, an `Email` column whose values lost their `@`. A file could be reported as
///   "every released row shares its combination of birth_date with at least 19 other(s)"
///   when all 20 birth dates were distinct and the projection returned an empty string
///   every time. Both a pass, and false.
/// - Yielded alone is too weak, because four projections never return an empty string: an
///   empty cell reads as `0:0` under `NumericIdWidth` and as `0` under `NameTokenCount`.
///   Three all-blank columns satisfied it and were named in a verified finding — "shares
///   the surviving format of c0, c1, c2" — about three columns holding nothing. A constant
///   projection separates nobody from anybody, whatever it is constant *at*.
#[derive(Debug, Clone)]
enum ProjectionWitness {
    /// No row has been projected yet.
    Unseen,
    /// Every row projected so far produced this same value, which may be empty.
    Constant(String),
    /// Two rows produced different values, so this column tells them apart.
    Varied,
}

impl ProjectionWitness {
    /// Whether any row produced a non-empty projection.
    ///
    /// Read only on a file of fewer than two rows, where nothing *can* vary: dropping every
    /// column there would report "nothing is matchable" about a row that is trivially unique.
    fn yielded(&self) -> bool {
        match self {
            Self::Unseen => false,
            Self::Constant(first) => !first.is_empty(),
            Self::Varied => true,
        }
    }

    fn is_varied(&self) -> bool {
        matches!(self, Self::Varied)
    }
}

impl CountedColumn {
    fn note(&mut self, projected: &str) {
        match &self.witness {
            // Settled: nothing a later row says can unsettle it. Dropping the held value
            // here also keeps a column of long values from carrying one for the whole run.
            ProjectionWitness::Varied => {}
            ProjectionWitness::Unseen => {
                self.witness = ProjectionWitness::Constant(projected.to_string());
            }
            ProjectionWitness::Constant(first) if first != projected => {
                self.witness = ProjectionWitness::Varied;
            }
            ProjectionWitness::Constant(_) => {}
        }
    }
}

/// Counts released rows into equivalence classes as the transform streams.
///
/// Holds counts rather than rows: the memory is proportional to distinct classes, and the
/// figures it reports — a smallest class, a count of singletons, a percentile — all fall
/// out of the class histogram once the last row has been seen. That is the same reason
/// `TransformState::column_value_distributions` is materialised in `report()` rather than
/// accumulated as a running total.
#[derive(Debug, Clone, Default)]
pub(crate) struct RowUniquenessTracker {
    /// Resolved once, from the first row that arrives, and then fixed except for each
    /// entry's `contributed` flag. Empty until then.
    counted: Vec<CountedColumn>,
    linkable_classes: HashMap<u128, u32>,
    all_column_classes: HashMap<u128, u32>,
    /// Rows fed to `linkable_classes`, and so the denominator of every figure drawn from
    /// it. Frozen if that histogram stops.
    rows_measured: usize,
    activated: bool,
    /// Set when `linkable_classes` passed the ceiling. This is the flag that makes the
    /// measurement incomplete, because every reported figure but one comes out of that
    /// histogram.
    linkable_stopped: bool,
    /// Set when `all_column_classes` passed the ceiling. Costs `distinct_rows_all_columns`
    /// and nothing else, so it is tracked apart: one flag for both meant that any file with
    /// more than two million distinct *rows* — which the joint measure handles fine, since
    /// its classes are coarser than rows by construction — threw a perfectly good joint
    /// measure away and printed "not measured" instead.
    all_columns_stopped: bool,
    /// One histogram per entry in `counted`, in the same order, each counting the classes the
    /// file would have had with that one column dropped. Empty whenever the attribution is
    /// not running.
    attribution: Vec<HashMap<u128, u32>>,
    /// Classes held across every map in `attribution`, maintained as they are inserted.
    ///
    /// Summing `len()` over the maps on every row would be the obvious alternative and is
    /// the reason this is a field: it is O(columns) work per row to answer a question that
    /// changes by at most one map entry per column per row.
    attribution_classes: usize,
    /// Set when the attribution will produce nothing: too many columns to track, the shared
    /// class budget spent, or the joint measurement itself stopped and left no baseline to
    /// compare a leave-one-out count against.
    attribution_stopped: bool,
}

impl RowUniquenessTracker {
    /// Records one released row.
    ///
    /// Takes the transformed row, not the source row: the file being handed over is the
    /// one whose rows matter, and reading the output is also what gives redaction and
    /// masking credit for the discriminating power they actually removed rather than the
    /// power the strategy is assumed to remove.
    pub(crate) fn record_row(&mut self, released: &[String], columns: &[ColumnMetadata]) {
        if !self.activated {
            self.activate(columns);
        }

        // Resolved once and then fixed, which is correct for every caller — a run's column
        // metadata is decided before its first row — and silently wrong for one that varied
        // it, since every position and index below would then describe a different column
        // than the value being read. Checked in debug rather than defended against, because
        // the honest repair for a varying shape is a second tracker, not a mid-run rebind.
        debug_assert!(
            self.counted.iter().all(|counted| columns
                .get(counted.position)
                .is_some_and(|column| column.index == counted.column_index
                    && LinkableProjection::for_column(column) == counted.projection)),
            "the tracker's columns were resolved from the first row and cannot be rebound"
        );

        if self.linkable_stopped {
            // The histogram is gone but the column list is still reported, so the flags behind
            // it have to stay true of the whole file. Only columns that have not yet varied
            // are re-checked, which on any real input is none of them after the first rows.
            for counted in self
                .counted
                .iter_mut()
                .filter(|counted| !counted.witness.is_varied())
            {
                // No `rows_yielded` here on purpose: this branch does not advance
                // `rows_measured`, and the two are compared against each other. Counting a
                // row here that the denominator never saw would make a column look partial
                // on a file where every row carried it.
                let value = released
                    .get(counted.position)
                    .and_then(|value| counted.projection.apply(value))
                    .unwrap_or(Cow::Borrowed(""));
                counted.note(&value);
            }
        } else {
            let projected = self
                .counted
                .iter_mut()
                .map(|counted| {
                    let projected = released
                        .get(counted.position)
                        .and_then(|value| counted.projection.apply(value));
                    // A row shorter than the metadata reaches `None` here too, and that is
                    // the right answer: a cell that is not in the row carries nothing the
                    // column was matched on.
                    if projected.is_some() {
                        counted.rows_yielded += 1;
                    }
                    let value = projected.unwrap_or(Cow::Borrowed(""));
                    counted.note(&value);
                    value
                })
                .collect::<Vec<_>>();
            let linkable_key = hash_fields(2, projected.iter().map(Cow::as_ref));

            self.rows_measured += 1;
            let class = self.linkable_classes.entry(linkable_key).or_insert(0);
            // Saturating, because wrapping would report the largest class in the file as the
            // smallest. Unreachable below 4.29 billion identical rows, and the cost of being
            // wrong is a `smallest_class` of 0 under a verified tick.
            *class = class.saturating_add(1);

            if !self.attribution_stopped {
                self.record_attribution(&projected);
            }

            if self.linkable_classes.len() > CLASS_CEILING {
                self.linkable_stopped = true;
                self.linkable_classes = HashMap::new();
                // The attribution answers "how many rows would still be unique", and there is
                // no longer a measured count of unique rows for that to be an answer *to*.
                // Kept running it would spend its budget deriving a comparison against a
                // figure the summary suppresses.
                self.stop_attribution();
            }
        }

        // Counted independently of the histogram above, and its own ceiling reached
        // independently, because this map fills far faster: its classes are whole rows
        // where the other's are projections of a subset of them.
        if !self.all_columns_stopped {
            let all_columns_key = hash_fields(1, released.iter().map(String::as_str));
            let class = self.all_column_classes.entry(all_columns_key).or_insert(0);
            *class = class.saturating_add(1);
            if self.all_column_classes.len() > CLASS_CEILING {
                self.all_columns_stopped = true;
                self.all_column_classes = HashMap::new();
            }
        }
    }

    fn activate(&mut self, columns: &[ColumnMetadata]) {
        self.counted = columns
            .iter()
            .enumerate()
            .map(|(position, column)| CountedColumn {
                position,
                column_index: column.index,
                projection: LinkableProjection::for_column(column),
                witness: ProjectionWitness::Unseen,
                rows_yielded: 0,
            })
            .collect();
        self.activated = true;

        // Sized once, here, so that `record_attribution` can zip the maps against the
        // projections without a bounds check and without ever growing the vector mid-file.
        //
        // The empty case is stopped rather than allocated, because an empty `attribution`
        // beside a `false` flag reports "we looked and no column helps" — the reading the flag
        // exists to prevent — about a run with no columns to look at. Unreachable through
        // `release_report`, which returns before the advice on an empty `matched_columns`, and
        // held here anyway so the flag's meaning does not depend on a caller's early return.
        if self.counted.is_empty() || self.counted.len() > ATTRIBUTION_COLUMN_CAP {
            self.attribution_stopped = true;
        } else {
            self.attribution = vec![HashMap::new(); self.counted.len()];
        }
    }

    /// Feeds one row's projections to the leave-one-out histograms.
    ///
    /// Each column's key is the row's total minus that column's own contribution, so all of
    /// them come out of one pass over `projected` rather than one re-hash of the row per
    /// column. The naive shape is quadratic in the column count, and on a wide file it is the
    /// difference between an attribution that runs and one that has to be capped away.
    ///
    /// `projected` is the same slice the joint key was built from, which is what makes the
    /// counts comparable: a leave-one-out histogram built over anything else would be
    /// answering a different question than the `unique_rows` it is subtracted from.
    fn record_attribution(&mut self, projected: &[Cow<'_, str>]) {
        let components = projected
            .iter()
            .enumerate()
            .map(|(position, value)| component_hash(position, value))
            .collect::<Vec<_>>();
        let total = components
            .iter()
            .fold(0u128, |sum, component| sum.wrapping_add(*component));

        let mut added = 0;
        for (classes, component) in self.attribution.iter_mut().zip(&components) {
            let before = classes.len();
            let class = classes.entry(total.wrapping_sub(*component)).or_insert(0);
            // Saturating for the same reason as the joint histogram: a wrapped count reads as
            // a class of one, and a class of one is the finding this whole module reports.
            *class = class.saturating_add(1);
            added += classes.len() - before;
        }
        self.attribution_classes += added;

        // Checked once per row rather than once per insertion, so the total can overshoot by
        // at most one entry per tracked column — twenty-four — before it stops. The ceiling is
        // a budget, not a boundary anything is derived from, and a per-insertion check would
        // put a branch in the inner loop to save 24 entries out of four million.
        if self.attribution_classes > ATTRIBUTION_CLASS_CEILING {
            self.stop_attribution();
        }
    }

    /// Gives up on the attribution and releases everything it was holding.
    ///
    /// Dropping the maps rather than keeping them is what makes the shared ceiling a real
    /// memory bound instead of a reporting rule: the figures are unusable the moment the
    /// histograms are partial, so holding them costs the memory the ceiling exists to cap
    /// and buys nothing.
    fn stop_attribution(&mut self) {
        self.attribution_stopped = true;
        self.attribution = Vec::new();
        self.attribution_classes = 0;
    }

    /// The columns the measure read, each paired with what it was matched on, in the order
    /// the columns appear in the metadata.
    ///
    /// A column is named only if its projection actually varied across the file, so neither
    /// a silent column nor a constant one can support a claim about itself. Below two rows
    /// nothing can vary, so there the weaker test applies — otherwise a one-row file would
    /// report that nothing is matchable about a row that is trivially unique.
    fn matched_columns(&self) -> Vec<MatchedColumn> {
        self.counted
            .iter()
            .filter(|counted| self.is_matched(counted))
            .map(|counted| MatchedColumn {
                column_index: counted.column_index,
                matched_on: counted.projection.matched_part(),
                // Compared against the rows this measure actually saw, not against the file's
                // row count: on an incomplete measurement the two differ, and the claim being
                // qualified is a claim about the measured rows.
                matched_every_row: counted.rows_yielded == self.rows_measured,
            })
            .collect()
    }

    /// Whether the report may say anything at all about `counted`.
    ///
    /// One predicate rather than two, because [`Self::matched_columns`] and
    /// [`Self::drop_column_effects`] are read side by side — the report pairs each effect
    /// with the name of a matched column — and two copies of this rule drifting apart would
    /// produce an effect for a column the reader was never told was matched, or advice to
    /// drop a column the finding does not rest on.
    fn is_matched(&self, counted: &CountedColumn) -> bool {
        if self.rows_measured < 2 {
            counted.witness.yielded()
        } else {
            counted.witness.is_varied()
        }
    }

    /// What `unique_rows` would have been with each matched column dropped, best first.
    ///
    /// Unmatched columns are left out rather than reported as having no effect. Their effect
    /// is genuinely nil — a constant projection puts every row in the same class whether it
    /// is read or not — so the row would be true and useless, and it would invite a reader to
    /// drop a column on the strength of a measure that never counted it.
    fn drop_column_effects(&self) -> Vec<DropColumnEffect> {
        let mut effects = self
            .counted
            .iter()
            .zip(&self.attribution)
            .filter(|(counted, _)| self.is_matched(counted))
            .map(|(counted, classes)| DropColumnEffect {
                column_index: counted.column_index,
                unique_rows_without: classes.values().filter(|count| **count == 1).count(),
            })
            .collect::<Vec<_>>();
        // Best first, so the report can quote the head of the list without re-deriving what
        // "best" means. Ties break on the column index so that two columns which help
        // equally come out in a fixed order rather than the hash map's.
        effects.sort_by_key(|effect| (effect.unique_rows_without, effect.column_index));
        effects
    }

    /// The end-of-run figures, or `None` when no row was ever recorded.
    ///
    /// `None` rather than a zeroed summary, because the paths that never call
    /// [`Self::record_row`] are the ones with no rows to speak of — unstructured text,
    /// a single pasted value — and a summary reading "0 unique rows" would be read as a
    /// clean bill of health issued by a check that never ran.
    pub(crate) fn summary(&self) -> Option<RowUniquenessSummary> {
        if !self.activated {
            return None;
        }

        let matched_columns = self.matched_columns();
        // Absent, not zero, when its own ceiling was reached. Zero distinct rows is a
        // figure no file has, and a reader has no way to tell a suppressed count from a
        // measured one once both read `0`.
        let distinct_rows_all_columns =
            (!self.all_columns_stopped).then_some(self.all_column_classes.len());

        if self.linkable_stopped {
            return Some(RowUniquenessSummary {
                rows_measured: self.rows_measured,
                matched_columns,
                distinct_rows_all_columns,
                measurement_incomplete: true,
                // `stop_attribution` already ran when the joint histogram stopped, so this is
                // restating what the tracker holds rather than deciding it. Written out
                // anyway: the field defaulting to `false` here would publish "we looked and
                // no column helps" about a file nothing was measured on.
                drop_attribution_incomplete: true,
                ..RowUniquenessSummary::default()
            });
        }

        let mut class_sizes = self
            .linkable_classes
            .values()
            .map(|count| *count as usize)
            .collect::<Vec<_>>();
        class_sizes.sort_unstable();

        Some(RowUniquenessSummary {
            rows_measured: self.rows_measured,
            matched_columns,
            distinct_classes: class_sizes.len(),
            unique_rows: class_sizes.iter().filter(|size| **size == 1).count(),
            smallest_class: class_sizes.first().copied().unwrap_or(0),
            fifth_percentile_class_size: fifth_percentile_class_size(
                &class_sizes,
                self.rows_measured,
            ),
            distinct_rows_all_columns,
            measurement_incomplete: false,
            drop_column_effects: if self.attribution_stopped {
                Vec::new()
            } else {
                self.drop_column_effects()
            },
            drop_attribution_incomplete: self.attribution_stopped,
        })
    }
}

/// The class size at or below which the smallest 5% of rows sit.
///
/// Reported next to the smallest class because the smallest class is one row's opinion:
/// a single freak record drags it to 1 on a file that is otherwise comfortably grouped,
/// and a reader who acts on that alone will either panic or, having panicked once too
/// often, stop reading. Walking 5% of the *rows* rather than 5% of the classes is what
/// makes this a statement about how exposed the population is rather than about how the
/// classes happen to be counted.
///
/// `class_sizes` must be sorted ascending.
fn fifth_percentile_class_size(class_sizes: &[usize], rows_measured: usize) -> usize {
    if class_sizes.is_empty() || rows_measured == 0 {
        return 0;
    }

    // Ceiling division, so a file small enough that 5% rounds to zero still asks about
    // its first row rather than reporting a percentile nothing was measured for.
    let target = rows_measured.div_ceil(20).max(1);
    let mut seen = 0;
    for size in class_sizes {
        seen += size;
        if seen >= target {
            return *size;
        }
    }

    class_sizes.last().copied().unwrap_or(0)
}

/// A 128-bit key over `fields`, length-prefixed and domain-separated.
///
/// 128 bits rather than 64 because of which way the error points. A collision merges two
/// genuinely different rows into one class, which makes classes look *larger* and the file
/// look *safer* — the one direction a privacy figure must not be wrong in. At 64 bits and
/// ten million rows the expected collision count is about 2.7e-3: small, but it is a silent
/// under-statement of risk, and a second hash costs one pass over data already in cache.
/// At 128 bits the same figure is around 1e-22 and can be dismissed honestly.
///
/// `DefaultHasher` is SipHash-1-3 and is not cryptographic. It does not need to be: this
/// defends against accidental collisions between the file's own rows, not against an
/// adversary who chooses rows to collide. Anyone able to do that already has the file.
///
/// Each field is written with its byte length in front so that `["ab", "c"]` and
/// `["a", "bc"]` cannot hash alike — without it, two different rows would share a class
/// whenever a separator moved, which is the same under-statement in a cheaper disguise.
/// `domain` separates the two histograms so the all-column and linkable keys of an
/// identical field list stay distinguishable.
fn hash_fields<'a>(domain: u8, fields: impl Iterator<Item = &'a str> + Clone) -> u128 {
    let low = hash_fields_with_seed(domain, 0, fields.clone());
    let high = hash_fields_with_seed(domain, 0x9E37_79B9_7F4A_7C15, fields);
    (u128::from(high) << 64) | u128::from(low)
}

/// One column's contribution to an additively-composed row key.
///
/// The leave-one-out attribution needs the key of a row *without* column `i`, for every `i`,
/// and [`hash_fields`] cannot give it one: a sequential hash has to be recomputed from
/// scratch for each omission, which is quadratic in the column count. Summing independent
/// per-column hashes composes instead — the key without column `i` is the row's total minus
/// that column's component — so every omission falls out of one pass. `wrapping_add` makes
/// the subtraction exact rather than approximate: the group is `u128` under addition, so
/// removing a component recovers the sum of the others precisely.
///
/// What the sum gives up is sequence. A sequential hash distinguishes `["a", "b"]` from
/// `["b", "a"]` for free; a sum does not, so the position is hashed *into* each component and
/// two columns holding the same projection get different contributions. The field's byte
/// length goes in for symmetry with [`hash_fields`], and is belt-and-braces rather than
/// load-bearing: `field.as_bytes()` hashes through `impl Hash for [T]`, which writes its own
/// length prefix, so `["ab", "c"]` and `["a", "bc"]` stay apart with or without it. Position
/// is the part that is load-bearing, and nothing else here supplies it. The width is the same
/// 128 bits, so the collision argument there carries over: components are uniform and
/// independent, so their sum is uniform.
///
/// Not *unchanged*, though, and the difference points the wrong way, which is why it is
/// written down. The attribution runs one histogram per tracked column, so a file gets up to
/// twenty-four independent chances at a merge rather than one, and its expectation is that
/// many times the figure quoted there. Twenty-four times a number around 1e-22 is still a
/// number nobody has to think about, which is the only reason this is a note and not a
/// redesign.
///
/// Its own domain, so an attribution key and a joint key over identical fields cannot
/// coincide. They are never compared, and separating them keeps that a property of the hash
/// rather than of the code that happens to read it.
fn component_hash(position: usize, field: &str) -> u128 {
    let low = component_hash_with_seed(position, 0, field);
    let high = component_hash_with_seed(position, 0x9E37_79B9_7F4A_7C15, field);
    (u128::from(high) << 64) | u128::from(low)
}

fn component_hash_with_seed(position: usize, seed: u64, field: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    3u8.hash(&mut hasher);
    position.hash(&mut hasher);
    field.len().hash(&mut hasher);
    field.as_bytes().hash(&mut hasher);
    hasher.finish()
}

fn hash_fields_with_seed<'a>(domain: u8, seed: u64, fields: impl Iterator<Item = &'a str>) -> u64 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    domain.hash(&mut hasher);
    for field in fields {
        field.len().hash(&mut hasher);
        field.as_bytes().hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests;
