//! A set of types and functions related to cryptography, that are widely used in the entire Simperby project.
use secp256k1::{
    ecdsa::{RecoverableSignature, RecoveryId},
    Message, Secp256k1, SecretKey,
};
use serde::{ser::SerializeTuple, Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::fmt;
use thiserror::Error;

const EVM_EC_RECOVERY_OFFSET: u8 = 27;

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

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Copy)]
pub struct HexSerializedBytes<const N: usize> {
    pub data: [u8; N],
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
        if serializer.is_human_readable() {
            serializer.serialize_str(hex::encode(self.data).as_str())
        } else {
            let mut seq = serializer.serialize_tuple(N)?;
            for e in self.data {
                seq.serialize_element(&e)?;
            }
            seq.end()
        }
    }
}

impl<const N: usize> fmt::Debug for HexSerializedBytes<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.data).as_str())
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
        if deserializer.is_human_readable() {
            let s: String = Deserialize::deserialize(deserializer)?;
            let bytes = hex::decode(s).map_err(|e| serde::de::Error::custom(e.to_string()))?;
            if bytes.len() != N {
                return Err(serde::de::Error::custom("invalid length"));
            }
            let mut data = [0; N];
            data.copy_from_slice(&bytes);
            Ok(HexSerializedBytes { data })
        } else {
            struct V<const M: usize>;
            impl<'de, const M: usize> serde::de::Visitor<'de> for V<M> {
                type Value = [u8; M];

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("byte")
                }

                fn visit_seq<S: serde::de::SeqAccess<'de>>(
                    self,
                    mut seq: S,
                ) -> Result<Self::Value, S::Error> {
                    let mut data = [0; M];
                    for (i, x) in data.iter_mut().enumerate() {
                        *x = seq
                            .next_element()?
                            .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                    }
                    Ok(data)
                }
            }
            let data = deserializer.deserialize_tuple(N, V::<N>)?;
            Ok(HexSerializedBytes { data })
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct HexSerializedVec {
    pub data: Vec<u8>,
}

impl From<Vec<u8>> for HexSerializedVec {
    fn from(data: Vec<u8>) -> Self {
        Self { data }
    }
}

impl<const N: usize> From<[u8; N]> for HexSerializedVec {
    fn from(data: [u8; N]) -> Self {
        Self {
            data: data.to_vec(),
        }
    }
}

impl<const N: usize> From<HexSerializedBytes<N>> for HexSerializedVec {
    fn from(data: HexSerializedBytes<N>) -> Self {
        Self {
            data: data.data.to_vec(),
        }
    }
}

impl Serialize for HexSerializedVec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(hex::encode(&self.data).as_str())
        } else {
            let mut seq = serializer.serialize_tuple(self.data.len())?;
            for e in &self.data {
                seq.serialize_element(&e)?;
            }
            seq.end()
        }
    }
}

impl fmt::Debug for HexSerializedVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.data).as_str())
    }
}

impl fmt::Display for HexSerializedVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.data).as_str())
    }
}

impl<'de> Deserialize<'de> for HexSerializedVec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let s: String = Deserialize::deserialize(deserializer)?;
            let data = hex::decode(s).map_err(|e| serde::de::Error::custom(e.to_string()))?;
            Ok(HexSerializedVec { data })
        } else {
            let data: Vec<u8> = Deserialize::deserialize(deserializer)?;
            Ok(HexSerializedVec { data })
        }
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
        let mut hasher = Keccak256::new();
        hasher.update(data);
        let result = hasher.finalize();
        Hash256 {
            hash: HexSerializedBytes {
                data: result.as_slice().try_into().unwrap(),
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
    signature: HexSerializedBytes<65>,
}

impl Signature {
    pub const fn zero() -> Self {
        Signature {
            signature: HexSerializedBytes { data: [0; 65] },
        }
    }

    /// Creates a new signature from the given data and keys.
    pub fn sign(data: Hash256, private_key: &PrivateKey) -> Result<Self, Error> {
        let private_key = secp256k1::SecretKey::from_slice(&private_key.key.data)
            .map_err(|_| Error::InvalidFormat("private key: [omitted]".to_owned()))?;
        let message = Message::from_slice(data.as_ref()).unwrap();
        let (recovery_id, rs) = Secp256k1::signing_only()
            .sign_ecdsa_recoverable(&message, &private_key)
            .serialize_compact();
        let v = recovery_id.to_i32() as u8;
        let bytes: [u8; 65] = {
            let mut whole: [u8; 65] = [0; 65];
            let (left, right) = whole.split_at_mut(rs.len());
            left.copy_from_slice(&rs);
            right.copy_from_slice(&[v + EVM_EC_RECOVERY_OFFSET; 1]);
            whole
        };
        Ok(Signature {
            signature: HexSerializedBytes { data: bytes },
        })
    }

    /// Verifies the signature against the given data and public key.
    pub fn verify(&self, data: Hash256, public_key: &PublicKey) -> Result<(), Error> {
        let signature = secp256k1::ecdsa::Signature::from_compact(&self.signature.data[0..64])
            .map_err(|_| Error::InvalidFormat(format!("signature: {self}")))?;
        let public_key = secp256k1::PublicKey::from_slice(&public_key.key.data)
            .map_err(|_| Error::InvalidFormat(format!("public_key: {public_key}")))?;
        let message = Message::from_slice(data.as_ref()).unwrap();
        Secp256k1::verification_only()
            .verify_ecdsa(&message, &signature, &public_key)
            .map_err(|_| Error::VerificationFailed)
    }

    /// Recover a public key from the given signature.
    pub fn recover(&self, data: Hash256) -> Result<PublicKey, Error> {
        let message = Message::from_slice(data.as_ref()).unwrap();
        let recovery_id = RecoveryId::from_i32(
            self.signature.data[64..65][0] as i32 - EVM_EC_RECOVERY_OFFSET as i32,
        )
        .map_err(|e| Error::InvalidFormat(e.to_string()))?;
        if recovery_id.to_i32() != 0 && recovery_id.to_i32() != 1 {
            return Err(Error::VerificationFailed);
        }
        let signature =
            RecoverableSignature::from_compact(&self.signature.data[0..64], recovery_id)
                .map_err(|e| Error::InvalidFormat(e.to_string()))?;
        let secp = Secp256k1::new();
        let public_key = secp
            .recover_ecdsa(&message, &signature)
            .map_err(|e| Error::InvalidFormat(e.to_string()))?
            .serialize();
        PublicKey::from_array(public_key)
    }

    /// Constructs a signature from the given bytes, but does not verify its validity.
    pub fn from_array(bytes: [u8; 65]) -> Self {
        Signature {
            signature: HexSerializedBytes { data: bytes },
        }
    }
}

/// A signature that is explicitly marked with the type of the signed data.
///
/// This implies that the signature is created on `Hash256::hash(serde_spb::to_vec(T).unwrap())`.
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

    pub fn get_raw_signature(&self) -> Signature {
        self.signature.clone()
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
    key: HexSerializedBytes<65>,
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

    pub fn from_array_uncompressed(array: [u8; 65]) -> Result<Self, Error> {
        let key = secp256k1::PublicKey::from_slice(array.as_ref())
            .map_err(|_| Error::InvalidFormat(format!("given bytes: {}", hex::encode(array))))?
            .serialize_uncompressed();
        Ok(PublicKey {
            key: HexSerializedBytes { data: key },
        })
    }

    pub fn from_array(array: [u8; 33]) -> Result<Self, Error> {
        let key = secp256k1::PublicKey::from_slice(array.as_ref())
            .map_err(|_| Error::InvalidFormat(format!("given bytes: {}", hex::encode(array))))?
            .serialize_uncompressed();
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
    pub fn zero() -> Self {
        Self {
            key: HexSerializedBytes::zero(),
        }
    }

    pub fn from_array(array: [u8; 32]) -> Result<Self, Error> {
        let key = secp256k1::SecretKey::from_slice(&array)
            .map_err(|_| Error::InvalidFormat(format!("given bytes: {}", hex::encode(array))))?
            .secret_bytes();
        Ok(PrivateKey {
            key: HexSerializedBytes { data: key },
        })
    }

    pub fn public_key(&self) -> PublicKey {
        let private_key = SecretKey::from_slice(&self.key.data).expect("invalid private key");
        let secp = Secp256k1::new();
        let public_key = private_key.public_key(&secp);
        PublicKey::from_array(public_key.serialize()).expect("invalid public key")
    }
}

/// Checks whether the given public and private keys match.
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
    use secp256k1::rand::SeedableRng;
    let mut rng = secp256k1::rand::rngs::StdRng::from_seed(seed_);
    let secp = Secp256k1::new();
    let (private_key, public_key) = secp.generate_keypair(&mut rng);
    (
        PublicKey::from_array(public_key.serialize()).expect("invalid public key"),
        PrivateKey::from_array(private_key.secret_bytes()).expect("invalid private key"),
    )
}

/// Generates a new keypair randomly
pub fn generate_keypair_random() -> (PublicKey, PrivateKey) {
    use secp256k1::rand::SeedableRng;
    let mut rng = secp256k1::rand::rngs::StdRng::from_entropy();
    let secp = Secp256k1::new();
    let (private_key, public_key) = secp.generate_keypair(&mut rng);
    (
        PublicKey::from_array(public_key.serialize()).expect("invalid public key"),
        PrivateKey::from_array(private_key.secret_bytes()).expect("invalid private key"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serde_spb;

    #[test]
    fn pretty_format() {
        let hash = Hash256::hash("hello world");
        assert_eq!(hash.to_string().len(), 64);
        let encoded = serde_spb::to_string(&hash).unwrap();
        assert_eq!(encoded.len(), 66);
        let (public_key, private_key) = generate_keypair("hello rustaceans");
        let signature = Signature::sign(hash, &private_key).unwrap();
        let encoded = serde_spb::to_string(&signature).unwrap();
        assert_eq!(encoded.len(), 132);
        let encoded = serde_spb::to_string(&public_key).unwrap();
        assert_eq!(encoded.len(), 132);
        let encoded = serde_spb::to_string(&private_key).unwrap();
        assert_eq!(encoded.len(), 66);
    }

    #[test]
    fn pretty_format2() {
        let hash = Hash256::zero();
        assert_eq!(hash.to_string().len(), 64);
        let encoded = serde_spb::to_string(&hash).unwrap();
        assert_eq!(encoded.len(), 66);
        let (public_key, private_key) = generate_keypair_random();
        let signature = Signature::sign(hash, &private_key).unwrap();
        let encoded = serde_spb::to_string(&signature).unwrap();
        assert_eq!(encoded.len(), 132);
        let encoded = serde_spb::to_string(&public_key).unwrap();
        assert_eq!(encoded.len(), 132);
        let encoded = serde_spb::to_string(&private_key).unwrap();
        assert_eq!(encoded.len(), 66);
    }

    #[test]
    fn hash_encode_decode() {
        let hash = Hash256::hash("hello world");
        let encoded = serde_spb::to_string(&hash).unwrap();
        let decoded = serde_spb::from_str(&encoded).unwrap();
        assert_eq!(hash, decoded);
    }

    #[test]
    fn hash_encode_decode_zero() {
        let hash = Hash256::zero();
        let encoded = serde_spb::to_string(&hash).unwrap();
        let decoded = serde_spb::from_str(&encoded).unwrap();
        assert_eq!(hash, decoded);
    }

    #[test]
    fn key_encode_decode() {
        let (public_key, private_key) = generate_keypair("hello world");
        let encoded = serde_spb::to_string(&public_key).unwrap();
        let decoded = serde_spb::from_str(&encoded).unwrap();
        assert_eq!(public_key, decoded);
        let encoded = serde_spb::to_string(&private_key).unwrap();
        let decoded = serde_spb::from_str(&encoded).unwrap();
        assert_eq!(private_key, decoded);
    }

    #[test]
    fn signature_encode_decode() {
        let (public_key, private_key) = generate_keypair("hello world");
        let signature = Signature::sign(Hash256::hash("hello world"), &private_key).unwrap();
        let encoded = serde_spb::to_string(&signature).unwrap();
        let decoded = serde_spb::from_str(&encoded).unwrap();
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

    #[test]
    fn signature_verify_invalid() {
        let (public_key, private_key) = generate_keypair("hello world");
        let signature = Signature::sign(Hash256::hash("hello world2"), &private_key).unwrap();
        signature
            .verify(Hash256::hash("hello world"), &public_key)
            .unwrap_err();
    }

    #[test]
    fn compressed() {
        let public_key = "0479c0e6973634b801da80fdf9274c13e327880e6360ca7735877f16e6a903c811afc2f0bb2c17de59110b022956dee0d625a694132b0da03fbba8ccdca219657c";
        let private_key = "f54a850441ef31968ffda8ea2ebdd831f0764c6bd52cd5185c8cb35d407201a4";
        let public_key = hex::decode(public_key).unwrap();
        let private_key = hex::decode(private_key).unwrap();
        let public_key =
            PublicKey::from_array_uncompressed(public_key.as_slice().try_into().unwrap()).unwrap();
        let private_key =
            PrivateKey::from_array(private_key.as_slice().try_into().unwrap()).unwrap();
        check_keypair_match(&public_key, &private_key).unwrap();
    }

    #[test]
    fn recover_public_key() {
        let (public_key, private_key) = generate_keypair("hello world");
        let signature = Signature::sign(Hash256::hash("hello world2"), &private_key).unwrap();
        let recovered = signature.recover(Hash256::hash("hello world2")).unwrap();
        assert_eq!(
            hex::encode(public_key.as_ref()),
            hex::encode(recovered.as_ref())
        );
    }
}
