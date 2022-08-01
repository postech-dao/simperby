use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simperby_common::crypto::*;
use simperby_common::*;
use simperby_kv_store::KVStore;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Block<T> {
    pub header: BlockHeader,
    pub transactions: Vec<T>,
}

/// A state transition function.
#[async_trait]
pub trait BlockExecutor {
    type Transaction;
    async fn execute(
        &self,
        store: &mut dyn KVStore,
        transaction: Self::Transaction,
    ) -> Result<(), ()>;
}

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

/// A consensus item that can be voted (thus signed), if the node operator is in favor of it.
///
/// Due to the 'interactive' nature of the Simperby consensus,
/// a typical block validator would manually read the content of `ConsensusVoteItem` and decide whether
/// it is favorable or not.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ConsensusVoteItem {
    /// The hash of the item, which is used as the unique identifier of the it and also used as the sign target.
    pub hash: Hash256,
    /// (If exists) The block which is associated with this vote item.
    pub block: Option<Block<Transaction>>,
    /// A human-readable description of the item.
    pub description: String,
}

#[async_trait]
pub trait SimperbyApi {
    /// Gets the current height of the blockchain.
    async fn get_height(&self) -> u64;

    /// Gets the finalized block for the given height.
    async fn get_block(&self, height: u64) -> Result<Block<Transaction>, String>;

    /// Checks the given block as the next block to be added to the current state.
    ///
    /// Fails if the block is invalid.
    async fn check_block(&self, block: Block<Transaction>) -> Result<(), String>;

    /// Attempts to propose a block for this round.
    ///
    /// It fails
    /// 1. with the same cause as `check_block`
    /// 2. if this node is not the current leader.
    /// 3. if this node has already proposed another block for this round.
    async fn propose_block(
        &self,
        block: Block<Transaction>,
        signature: Signature,
    ) -> Result<(), String>;

    /// Reads the finlized state entry by the given key.
    async fn read_state(&self, key: String, height: u64) -> Result<String, String>;

    /// Gets the current possible consensus voting options.
    async fn get_consensus_vote_options(&self) -> Vec<ConsensusVoteItem>;

    /// Gets the current status of the ongoing consensus.
    ///
    /// This is essential because Simperby's consensus is 'interactive', which means that the validator
    /// has to understand what's going on and manually decide what to do regarding the consensus.
    ///
    /// TODO: define the type of the state.
    async fn get_consensus_status(&self) -> ();

    /// Submits a vote for the given item, identified by its hash.
    async fn submit_consensus_vote(
        &self,
        hash: Hash256,
        signature: Signature,
    ) -> Result<(), String>;
}
