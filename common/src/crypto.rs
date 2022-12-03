//! A set of types and functions related to cryptography, that are widely used in the entire Simperby project.
use ed25519::signature::{Signer, Verifier};
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum CryptoError {
    /// When the data format is not valid.
    #[error("invalid format: {0}")]
    InvalidFormat(String),
    #[error("verification failed")]
    VerificationFailed,
}

type Error = CryptoError;

pub trait ToHash256 {
    fn to_hash256(&self) -> Hash256;
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Copy)]
pub struct HexSerializedBytes<const N: usize> {
    data: [u8; N],
}

impl<const N: usize> HexSerializedBytes<N> {
    const fn zero() -> Self {
        Self { data: [0; N] }
    }
}

impl<const N: usize> Serialize for HexSerializedBytes<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(hex::encode(self.data).as_str())
    }
}

impl<const N: usize> fmt::Display for HexSerializedBytes<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.data).as_str())
    }
}

impl<'de, const N: usize> Deserialize<'de> for HexSerializedBytes<N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        let bytes = hex::decode(s).map_err(|e| serde::de::Error::custom(e.to_string()))?;
        if bytes.len() != N {
            return Err(serde::de::Error::custom("invalid length"));
        }
        let mut data = [0; N];
        data.copy_from_slice(&bytes);
        Ok(HexSerializedBytes { data })
    }
}

/// A cryptographic hash.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Copy, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Hash256 {
    pub hash: HexSerializedBytes<32>,
}

impl Hash256 {
    pub const fn zero() -> Self {
        Hash256 {
            hash: HexSerializedBytes::zero(),
        }
    }

    /// Hashes the given data.
    pub fn hash(data: impl AsRef<[u8]>) -> Self {
        Hash256 {
            hash: HexSerializedBytes {
                data: *blake3::hash(data.as_ref()).as_bytes(),
            },
        }
    }

    pub fn from_array(data: [u8; 32]) -> Self {
        Hash256 {
            hash: HexSerializedBytes { data },
        }
    }

    pub fn aggregate(&self, other: &Self) -> Self {
        Self::hash([self.hash.data, other.hash.data].concat())
    }
}

impl std::convert::AsRef<[u8]> for Hash256 {
    fn as_ref(&self) -> &[u8] {
        &self.hash.data
    }
}

impl fmt::Display for Hash256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.hash)
    }
}

/// A cryptographic signature.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Signature {
    signature: HexSerializedBytes<64>,
}

impl Signature {
    pub const fn zero() -> Self {
        Signature {
            signature: HexSerializedBytes { data: [0; 64] },
        }
    }

    /// Creates a new signature from the given data and keys.
    pub fn sign(data: Hash256, private_key: &PrivateKey) -> Result<Self, Error> {
        let private_key = ed25519_dalek::SecretKey::from_bytes(&private_key.key.data)
            .map_err(|_| Error::InvalidFormat("private key: [omitted]".to_owned()))?;
        let public_key: ed25519_dalek::PublicKey = (&private_key).into();
        let keypair = ed25519_dalek::Keypair {
            secret: private_key,
            public: public_key,
        };
        Ok(Signature {
            signature: HexSerializedBytes {
                data: keypair.sign(data.as_ref()).to_bytes(),
            },
        })
    }

    /// Verifies the signature against the given data and public key.
    pub fn verify(&self, data: Hash256, public_key: &PublicKey) -> Result<(), Error> {
        let signature = ed25519_dalek::Signature::from_bytes(&self.signature.data)
            .map_err(|_| Error::InvalidFormat(format!("signature: {}", self)))?;
        let public_key = ed25519_dalek::PublicKey::from_bytes(&public_key.key.data)
            .map_err(|_| Error::InvalidFormat(format!("public_key: {}", public_key)))?;
        public_key
            .verify(data.as_ref(), &signature)
            .map_err(|_| Error::VerificationFailed)
    }

    /// Constructs a signature from the given bytes, but does not verify its validity.
    pub fn from_array(bytes: [u8; 64]) -> Self {
        Signature {
            signature: HexSerializedBytes { data: bytes },
        }
    }
}

/// A signature that is explicitly marked with the type of the signed data.
///
/// This implies that the signature is created on `Hash256::hash(serde_json::to_vec(T).unwrap())`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Serialize, Deserialize)]
pub struct TypedSignature<T> {
    signature: Signature,
    signer: PublicKey,
    #[serde(skip)]
    _mark: std::marker::PhantomData<T>,
}

impl<T: ToHash256> TypedSignature<T> {
    /// Creates a new signature from the given data and keys.
    pub fn sign(data: &T, private_key: &PrivateKey) -> Result<Self, Error> {
        let data = data.to_hash256();
        Signature::sign(data, private_key).map(|signature| TypedSignature {
            signature,
            signer: private_key.public_key(),
            _mark: std::marker::PhantomData,
        })
    }

    pub fn new(signature: Signature, signer: PublicKey) -> Self {
        TypedSignature {
            signature,
            signer,
            _mark: std::marker::PhantomData,
        }
    }

    pub fn signer(&self) -> &PublicKey {
        &self.signer
    }

    /// Verifies the signature against the given data and public key.
    pub fn verify(&self, data: &T) -> Result<(), Error> {
        let data = data.to_hash256();
        self.signature.verify(data, &self.signer)
    }
}

impl std::convert::AsRef<[u8]> for Signature {
    fn as_ref(&self) -> &[u8] {
        &self.signature.data
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.signature)
    }
}

/// A public key.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PublicKey {
    key: HexSerializedBytes<32>,
}

impl std::convert::AsRef<[u8]> for PublicKey {
    fn as_ref(&self) -> &[u8] {
        &self.key.data
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.key)
    }
}

impl PublicKey {
    pub fn zero() -> Self {
        Self {
            key: HexSerializedBytes::zero(),
        }
    }

    pub fn from_array(array: [u8; 32]) -> Result<Self, Error> {
        let key = ed25519_dalek::PublicKey::from_bytes(array.as_ref())
            .map_err(|_| Error::InvalidFormat(format!("given bytes: {}", hex::encode(array))))?
            .to_bytes();
        Ok(PublicKey {
            key: HexSerializedBytes { data: key },
        })
    }
}

/// A private key.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PrivateKey {
    pub key: HexSerializedBytes<32>,
}

impl std::convert::AsRef<[u8]> for PrivateKey {
    fn as_ref(&self) -> &[u8] {
        &self.key.data
    }
}

impl PrivateKey {
    pub fn from_array(array: &[u8]) -> Result<Self, Error> {
        let key = ed25519_dalek::SecretKey::from_bytes(array)
            .map_err(|_| Error::InvalidFormat(format!("given bytes: {}", hex::encode(array))))?
            .to_bytes();
        Ok(PrivateKey {
            key: HexSerializedBytes { data: key },
        })
    }

    pub fn public_key(&self) -> PublicKey {
        let private_key =
            ed25519_dalek::SecretKey::from_bytes(&self.key.data).expect("private key is invalid");
        let public_key: ed25519_dalek::PublicKey = (&private_key).into();
        PublicKey::from_array(public_key.to_bytes()).expect("public key is invalid")
    }
}

/// Checkes whether the given public and private keys match.
pub fn check_keypair_match(public_key: &PublicKey, private_key: &PrivateKey) -> Result<(), Error> {
    let msg = "Some Random Message".as_bytes();
    let signature = Signature::sign(Hash256::hash(msg), private_key)?;
    signature.verify(Hash256::hash(msg), public_key)
}

/// Generates a new keypair using the seed.
pub fn generate_keypair(seed: impl AsRef<[u8]>) -> (PublicKey, PrivateKey) {
    let mut seed_: [u8; 32] = [0; 32];
    for (i, x) in Hash256::hash(seed).as_ref()[0..32].iter().enumerate() {
        seed_[i] = *x;
    }
    let mut rng = rand::rngs::StdRng::from_seed(seed_);
    let signing_key = ed25519_dalek::Keypair::generate(&mut rng);
    (
        PublicKey::from_array(signing_key.public.to_bytes()).expect("public key is invalid"),
        PrivateKey::from_array(&signing_key.secret.to_bytes()).expect("private key is invalid"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_format() {
        let hash = Hash256::hash("hello world");
        assert_eq!(hash.to_string().len(), 64);
        let encoded = serde_json::to_string(&hash).unwrap();
        assert_eq!(encoded.len(), 66);
        let (public_key, private_key) = generate_keypair("hello world");
        let signature = Signature::sign(hash, &private_key).unwrap();
        let encoded = serde_json::to_string(&signature).unwrap();
        assert_eq!(encoded.len(), 130);
        let encoded = serde_json::to_string(&public_key).unwrap();
        assert_eq!(encoded.len(), 66);
        let encoded = serde_json::to_string(&private_key).unwrap();
        assert_eq!(encoded.len(), 66);
    }

    #[test]
    fn hash_encode_decode() {
        let hash = Hash256::hash("hello world");
        let encoded = serde_json::to_string(&hash).unwrap();
        let decoded = serde_json::from_str(&encoded).unwrap();
        assert_eq!(hash, decoded);
    }

    #[test]
    fn hash_encode_decode_zero() {
        let hash = Hash256::zero();
        let encoded = serde_json::to_string(&hash).unwrap();
        let decoded = serde_json::from_str(&encoded).unwrap();
        assert_eq!(hash, decoded);
    }

    #[test]
    fn key_encode_decode() {
        let (public_key, private_key) = generate_keypair("hello world");
        let encoded = serde_json::to_string(&public_key).unwrap();
        let decoded = serde_json::from_str(&encoded).unwrap();
        assert_eq!(public_key, decoded);
        let encoded = serde_json::to_string(&private_key).unwrap();
        let decoded = serde_json::from_str(&encoded).unwrap();
        assert_eq!(private_key, decoded);
    }

    #[test]
    fn signature_encode_decode() {
        let (public_key, private_key) = generate_keypair("hello world");
        let signature = Signature::sign(Hash256::hash("hello world"), &private_key).unwrap();
        let encoded = serde_json::to_string(&signature).unwrap();
        let decoded = serde_json::from_str(&encoded).unwrap();
        assert_eq!(signature, decoded);
        signature
            .verify(Hash256::hash("hello world"), &public_key)
            .unwrap();
    }

    #[test]
    fn signature_verify() {
        let (public_key, private_key) = generate_keypair("hello world");
        let signature = Signature::sign(Hash256::hash("hello world"), &private_key).unwrap();
        signature
            .verify(Hash256::hash("hello world"), &public_key)
            .unwrap();
    }
}
