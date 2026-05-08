use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use thiserror::Error;

pub const CAPTURE_SCHEMA: &str = "larql.attention_decoupling.capture.v1";
pub const CAPSULE_SCHEMA: &str = "larql.attention_decoupling.capsule.v1";

#[derive(Debug, Error)]
pub enum CapsuleError {
    #[error("{0}")]
    Contract(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Capture<T> {
    pub schema_version: String,
    pub capture_kind: String,
    pub payload: T,
    pub payload_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Capsule<T> {
    pub schema_version: String,
    pub capsule_kind: String,
    pub producer: String,
    pub capture: Capture<T>,
    pub capsule_sha256: String,
}

#[derive(Serialize)]
struct CapsuleHashBody<'a, T> {
    schema_version: &'a str,
    capsule_kind: &'a str,
    producer: &'a str,
    capture: &'a Capture<T>,
}

pub fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, CapsuleError> {
    let value = serde_json::to_value(value)?;
    Ok(serde_json::to_vec(&value)?)
}

pub fn json_sha256<T: Serialize>(value: &T) -> Result<String, CapsuleError> {
    Ok(sha256_hex(&canonical_json_bytes(value)?))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

pub fn make_capture<T: Serialize>(
    capture_kind: impl Into<String>,
    payload: T,
) -> Result<Capture<T>, CapsuleError> {
    let payload_sha256 = json_sha256(&payload)?;
    Ok(Capture {
        schema_version: CAPTURE_SCHEMA.to_string(),
        capture_kind: capture_kind.into(),
        payload,
        payload_sha256,
    })
}

pub fn validate_capture<T: Serialize>(
    capture: &Capture<T>,
    capture_kind: Option<&str>,
) -> Result<(), CapsuleError> {
    if capture.schema_version != CAPTURE_SCHEMA {
        return Err(CapsuleError::Contract("capture.schema_version".to_string()));
    }
    if let Some(expected) = capture_kind {
        if capture.capture_kind != expected {
            return Err(CapsuleError::Contract("capture.capture_kind".to_string()));
        }
    }
    let expected = json_sha256(&capture.payload)?;
    if capture.payload_sha256 != expected {
        return Err(CapsuleError::Contract("capture.payload_sha256".to_string()));
    }
    Ok(())
}

pub fn make_capsule<T: Serialize>(
    capsule_kind: impl Into<String>,
    capture: Capture<T>,
    producer: impl Into<String>,
) -> Result<Capsule<T>, CapsuleError> {
    validate_capture(&capture, None)?;
    let capsule_kind = capsule_kind.into();
    let producer = producer.into();
    let body = CapsuleHashBody {
        schema_version: CAPSULE_SCHEMA,
        capsule_kind: &capsule_kind,
        producer: &producer,
        capture: &capture,
    };
    let capsule_sha256 = json_sha256(&body)?;
    Ok(Capsule {
        schema_version: CAPSULE_SCHEMA.to_string(),
        capsule_kind,
        producer,
        capture,
        capsule_sha256,
    })
}

pub fn validate_capsule<T: Serialize>(
    capsule: &Capsule<T>,
    capsule_kind: Option<&str>,
    capture_kind: Option<&str>,
) -> Result<(), CapsuleError> {
    if capsule.schema_version != CAPSULE_SCHEMA {
        return Err(CapsuleError::Contract("capsule.schema_version".to_string()));
    }
    if let Some(expected) = capsule_kind {
        if capsule.capsule_kind != expected {
            return Err(CapsuleError::Contract("capsule.capsule_kind".to_string()));
        }
    }
    validate_capture(&capsule.capture, capture_kind)?;
    let body = CapsuleHashBody {
        schema_version: &capsule.schema_version,
        capsule_kind: &capsule.capsule_kind,
        producer: &capsule.producer,
        capture: &capsule.capture,
    };
    let expected = json_sha256(&body)?;
    if capsule.capsule_sha256 != expected {
        return Err(CapsuleError::Contract("capsule.capsule_sha256".to_string()));
    }
    Ok(())
}

pub fn write_capsule_json<T: Serialize>(
    path: &Path,
    capsule: &Capsule<T>,
) -> Result<(), CapsuleError> {
    validate_capsule(capsule, None, None)?;
    let mut bytes = canonical_json_bytes(capsule)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes)?;
    Ok(())
}

pub fn read_capsule_json<T: DeserializeOwned + Serialize>(
    path: &Path,
) -> Result<Capsule<T>, CapsuleError> {
    let text = std::fs::read_to_string(path)?;
    let capsule: Capsule<T> = serde_json::from_str(&text)?;
    validate_capsule(&capsule, None, None)?;
    Ok(capsule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn capture_and_capsule_hashes_validate() {
        let capture = make_capture("unit_capture", json!({"b": 2, "a": 1})).unwrap();
        validate_capture(&capture, Some("unit_capture")).unwrap();
        let capsule = make_capsule("unit_capsule", capture, "unit").unwrap();
        validate_capsule(&capsule, Some("unit_capsule"), Some("unit_capture")).unwrap();
    }

    #[test]
    fn capture_hash_rejects_payload_mutation() {
        let mut capture = make_capture("unit_capture", json!({"a": 1})).unwrap();
        capture.payload = json!({"a": 2});
        assert!(validate_capture(&capture, Some("unit_capture")).is_err());
    }
}
