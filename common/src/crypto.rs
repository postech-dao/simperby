//! A set of types and functions related to cryptography, that are widely used in the entire Simperby project.
use serde::{Deserialize, Serialize};
use std::fmt;

/// A cryptographic hash.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize)]
pub struct Hash256 {
    pub dummy: String,
}

impl Hash256 {
    /// Hashes the given data.
    pub fn hash(_data: impl AsRef<[u8]>) -> Self {
        Hash256 {
            dummy: "".to_string(),
        }
    }
}

impl fmt::Display for Hash256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?")
    }
}

/// A cryptographic signature.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub dummy: String,
}

impl Signature {
    /// Creates a new signature from the given data and keys.
    pub fn sign(_data: Hash256, _public_key: &PublicKey, _private_key: &PrivateKey) -> Self {
        Signature {
            dummy: "".to_string(),
        }
    }

    /// Verifies the signature against the given data and public key.
    pub fn verify(&self, _data: Hash256, _public_key: &PublicKey) -> bool {
        true
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?")
    }
}

/// A public key.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize)]
pub struct PublicKey {
    pub dummy: String,
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?")
    }
}

/// A private key.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize)]
pub struct PrivateKey {
    pub dummy: String,
}

/// Checkes whether the given public and private keys match.
pub fn check_keypair_match(_public_key: &PublicKey, _private_key: &PrivateKey) -> bool {
    true
}

/// Generates a new keypair using the seed.
pub fn generate_keypair(_seed: impl AsRef<[u8]>) -> (PublicKey, PrivateKey) {
    (
        PublicKey {
            dummy: "".to_string(),
        },
        PrivateKey {
            dummy: "".to_string(),
        },
    )
}
