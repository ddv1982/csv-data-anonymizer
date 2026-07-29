//! Regex bodies shared by the column detectors and the inline span scanners.
//!
//! The two paths need the same shape with different anchoring: a column
//! detector must match a whole value (`^…$`), a span scanner must find the shape
//! inside free text (`\b…\b`). Keeping a copy on each side is how the column and
//! span paths drift apart on what counts as a UUID or a timestamp, so the body
//! lives here once and each side wraps it.
//!
//! Bodies use non-capturing groups throughout; every consumer only asks whether
//! the pattern matches and where, never for sub-captures.
//!
//! A shape only belongs here once both paths want it. Anything checksum- or
//! library-validated (IBAN, VAT, payment card, phone) stays out: those are
//! validators, not shapes, and their regexes are candidate filters feeding a
//! validator rather than the decision itself.

use regex::Regex;

pub(super) const UUID: &str =
    r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}";

pub(super) const TIMESTAMP: &str = r"\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?)?";

pub(super) const MAC_ADDRESS: &str = r"(?:[0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}";

/// A dotted-quad IPv4 address, rejecting leading zeros in an octet.
///
/// `010.0.0.1` is not a second spelling of `10.0.0.1`: some resolvers read a
/// leading zero as octal and route it to `8.0.0.1` instead. A value that means
/// different things to different readers is not an address, so neither path here
/// calls it one — and both paths use this body so they cannot disagree about it,
/// which they previously did in both directions.
pub(super) const IPV4: &str =
    r"(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)";

pub(super) const US_TAX_ID: &str = r"(?:\d{3}-\d{2}-\d{4}|\d{2}-\d{7})";

/// Matches only when `body` covers the entire value.
pub(super) fn whole_value(body: &str) -> Regex {
    Regex::new(&format!("^{body}$")).expect("shared pattern body should compile anchored")
}

/// Matches `body` anywhere it stands as its own word inside a larger string.
pub(super) fn inside_text(body: &str) -> Regex {
    Regex::new(&format!(r"\b{body}\b")).expect("shared pattern body should compile inline")
}
