use std::fmt::Debug;

use super::*;
use serde::{de::DeserializeOwned, ser::Serialize};

pub type DmsKey = String;

pub trait DmsMessage:
    Send + Sync + 'static + ToHash256 + Serialize + DeserializeOwned + Debug
{
    /// The tag for the DMS instance that handles this message.
    ///
    /// The DMS key will be deduced using this data and the hash of the last finalized header.
    const DMS_TAG: &'static str;

    /// Checks if the message is valid.
    fn check(&self) -> Result<(), Error>;

    /// Defines how to commit a message, by cryptographically signing it.
    ///
    /// In case that the message can't be guaranteed to be unique among other protocols,
    /// this method provides `dms_key` to be used as a unique identifier.
    ///
    /// One potential use case other than the default implementation which ignores `dms_key`
    /// is when the messages of which the signature is presented to another protocol,
    /// so not aggregating `dms_key`, (which is a very specific implementation detail of of DMS)
    /// makes sense.
    /// Of course, the message must be guaranteed to be unique so that the signature can't be replayed
    /// on another height or another chain.
    fn commit(
        &self,
        dms_key: &DmsKey,
        private_key: &PrivateKey,
    ) -> Result<MessageCommitmentProof, CryptoError>
    where
        Self: Sized,
    {
        let hash = self.to_hash256();
        Signature::sign(hash.aggregate(&dms_key.to_hash256()), private_key).map(|signature| {
            MessageCommitmentProof {
                committer: private_key.public_key(),
                signature,
            }
        })
    }

    /// It must match the `commit()` method if you implemented it.
    fn verify_commitment(
        &self,
        proof: &MessageCommitmentProof,
        dms_key: &DmsKey,
    ) -> Result<(), CryptoError> {
        proof.signature.verify(
            self.to_hash256().aggregate(&dms_key.to_hash256()),
            &proof.committer,
        )
    }
}

/// A message that the user of DMS observes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message<T: DmsMessage> {
    pub message: T,
    pub committers: Vec<MessageCommitmentProof>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageCommitmentProof {
    pub committer: PublicKey,
    pub signature: Signature,
}

/// The physical packet that is sent over the network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Packet {
    /// The original message data encoded in `serde_spb`.
    pub message: Vec<u8>,
    /// Commitment to the message with the proof.
    pub commitment: MessageCommitmentProof,
}

impl ToHash256 for Packet {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub message_hash: Hash256,
    pub committers: Vec<MessageCommitmentProof>,
}
