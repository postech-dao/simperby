use serde::{Deserialize, Serialize};
use simperby_common::crypto::*;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum StateTransition {
    InsertValidator {
        /// The public key of the validator.
        public_key: PublicKey,
        /// The weight of the validator.
        weight: u64,
    },
    RemoveValidator(PublicKey),
    /// This is accepted only if the signer of the transaction equals to the `delegator`.
    Delegate {
        /// The public key of the validator who delegates its voting right.
        delegator: PublicKey,
        /// The public key of the validator who is being delegated.
        delegatee: PublicKey,
        /// The target height of the block of this transaction, which is for preventing the replay attack.
        target_height: u64,
    },
    /// This is accepted only if the signer of the transaction equals to the `delegator`.
    Undelegate {
        /// The public key of the validator who claims its voting right.
        delegator: PublicKey,
        /// The target height of the block of this transaction, which is for preventing the replay attack.
        target_height: u64,
    },
    InsertData {
        key: String,
        value: String,
    },
    RemoveData(String),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Transaction {
    /// The siganture of this transaction.
    pub signature: Signature,
    /// The instruction to perform on the blockchain state.
    pub state_transition: Option<StateTransition>,
    /// An optional field to store data, which is not part of the state but still useful as it can be verified with the Merkle root.
    ///
    /// Note that it must not be `None` if the `state_transition` is `None`, which just makes the transaction pointless.
    pub data: Option<String>,
}
