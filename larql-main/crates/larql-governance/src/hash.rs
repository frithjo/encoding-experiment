use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct HashRef(String);

impl HashRef {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if is_hash_ref(&value) {
            Ok(Self(value))
        } else {
            Err(format!("invalid hash reference: {value}"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HashRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub fn hash_text(text: &str) -> String {
    hash_bytes(text.as_bytes())
}

pub fn hash_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(&serde_json::to_value(value)?)?;
    Ok(hash_bytes(&bytes))
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity("sha256:".len() + digest.len() * 2);
    out.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

pub fn is_hash_ref(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_ref_accepts_sha256_values() {
        let value = hash_text("hello");
        assert!(HashRef::new(value).is_ok());
    }

    #[test]
    fn hash_ref_rejects_raw_prose() {
        assert!(HashRef::new("this is not admitted authority").is_err());
    }
}
