//! Random value primitives shared by every replacement generator.
//!
//! Both the strategy transforms and the quick-generate path draw from here, so
//! there is one implementation of "n random characters from a charset" rather
//! than a copy per call site.

use rand::Rng;

pub(crate) const DIGIT_CHARSET: &str = "0123456789";

/// Digits a number may start with without changing how wide it reads.
pub(crate) const LEADING_DIGIT_CHARSET: &str = "123456789";

/// `length` characters drawn uniformly from `charset`.
///
/// Returns an empty string for an empty charset rather than panicking; callers
/// pass compile-time constants, but a generator that panics mid-file would
/// abort a transform that is already partway through writing output.
pub(crate) fn random_string(length: usize, charset: &str) -> String {
    let characters: Vec<char> = charset.chars().collect();
    if characters.is_empty() {
        return String::new();
    }
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| characters[rng.gen_range(0..characters.len())])
        .collect()
}

/// `length` random decimal digits, leading zeros allowed.
pub(crate) fn random_digits(length: usize) -> String {
    random_string(length, DIGIT_CHARSET)
}

/// One random decimal digit as a string.
pub(crate) fn random_digit() -> String {
    random_digits(1)
}

pub(crate) fn random_uuid_v4() -> String {
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
