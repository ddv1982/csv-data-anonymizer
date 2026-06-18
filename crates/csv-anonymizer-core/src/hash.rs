use sha2::{Digest, Sha256};

pub fn deterministic_hash(value: &str, seed: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{seed}:{value}").as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn deterministic_number(value: &str, seed: &str, min: i64, max: i64) -> i64 {
    let hash = deterministic_hash(value, seed);
    let hash_num = u64::from_str_radix(&hash[..8], 16).unwrap_or(0);
    let range = (max - min + 1).max(1) as u64;
    min + (hash_num % range) as i64
}

pub fn deterministic_string(value: &str, seed: &str, length: usize, charset: &str) -> String {
    let hash = deterministic_hash(value, seed);
    let chars: Vec<char> = charset.chars().collect();
    if chars.is_empty() {
        return String::new();
    }

    (0..length)
        .map(|index| {
            let start = (index * 2) % 64;
            let end = (start + 2).min(hash.len());
            let hex_pair = &hash[start..end];
            let char_index = usize::from_str_radix(hex_pair, 16).unwrap_or(0) % chars.len();
            chars[char_index]
        })
        .collect()
}

pub fn deterministic_uuid(value: &str, seed: &str) -> String {
    let hash = deterministic_hash(value, seed);
    let mut part3 = hash[12..16].to_string();
    part3.replace_range(0..1, "4");

    let variant_char = u8::from_str_radix(&hash[16..17], 16).unwrap_or(0);
    let variant_bits = (variant_char & 0x3) | 0x8;
    let mut part4 = hash[16..20].to_string();
    part4.replace_range(0..1, &format!("{variant_bits:x}"));

    format!(
        "{}-{}-{}-{}-{}",
        &hash[0..8],
        &hash[8..12],
        part3,
        part4,
        &hash[20..32]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_hash_matches_typescript_contract() {
        assert_eq!(
            deterministic_hash("test", "seed"),
            "a80700546770fb2775577dccb0e715cbf4c5f893f18f9a1468883336b0262405"
        );
    }

    #[test]
    fn deterministic_uuid_is_v4() {
        let uuid = deterministic_uuid("test", "seed");
        assert_eq!(uuid.chars().nth(14), Some('4'));
        assert!(matches!(uuid.chars().nth(19), Some('8' | '9' | 'a' | 'b')));
    }
}
