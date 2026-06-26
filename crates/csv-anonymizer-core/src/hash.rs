use sha2::{Digest, Sha256};

pub(crate) fn random_uuid_v4() -> String {
    use rand::Rng;

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

pub fn deterministic_hash(value: &str, seed: &str) -> String {
    let mut key = seed.as_bytes().to_vec();
    if key.len() > 64 {
        key = Sha256::digest(&key).to_vec();
    }
    key.resize(64, 0);

    let mut inner_key = [0x36_u8; 64];
    let mut outer_key = [0x5c_u8; 64];
    for (index, key_byte) in key.iter().enumerate() {
        inner_key[index] ^= key_byte;
        outer_key[index] ^= key_byte;
    }

    let mut inner = Sha256::new();
    inner.update(inner_key);
    inner.update(value.as_bytes());
    let inner_digest = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(outer_key);
    outer.update(inner_digest);
    format!("{:x}", outer.finalize())
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
    fn deterministic_hash_uses_hmac_sha256_contract() {
        assert_eq!(
            deterministic_hash("test", "seed"),
            "cafc65071bb5fb3cab17d223b140a52bd0417862ccc9939d6be10cfe408dc957"
        );
    }

    #[test]
    fn deterministic_uuid_is_v4() {
        let uuid = deterministic_uuid("test", "seed");
        assert_eq!(uuid.chars().nth(14), Some('4'));
        assert!(matches!(uuid.chars().nth(19), Some('8' | '9' | 'a' | 'b')));
    }
}
