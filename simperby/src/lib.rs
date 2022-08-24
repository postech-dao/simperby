#![allow(dead_code)]
pub mod node;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simperby_common::crypto::*;
use simperby_common::*;
use simperby_kv_storage::KVStorage;
use simperby_network::AuthorizedNetwork;
use thiserror::Error;
use vetomint::Round;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

/// A state transition function.
#[async_trait]
pub trait BlockExecutor {
    type Transaction;
    async fn execute(
        &self,
        storage: &mut dyn KVStorage,
        transaction: Self::Transaction,
    ) -> Result<(), ()>;
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
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
        value: Vec<u8>,
    },
    RemoveData(String),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Transaction {
    /// The siganture of this transaction.
    ///
    /// This is required when `state_transition` is `Delegate` or `Undelegate`.
    /// Otherwise, it must be `None`.
    pub signature: Option<Signature>,
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
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct ConsensusVoteItem {
    /// The hash of the item, which is used as the unique identifier of the it and also used as the sign target.
    pub hash: Hash256,
    /// (If exists) The block which is associated with this vote item.
    pub block: Option<Block>,
    /// A human-readable description of the item.
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct GenesisInfo {
    pub header: BlockHeader,
    pub genesis_signature: FinalizationProof,
    pub chain_name: String,
}

impl GenesisInfo {
    fn create_genesis_block(&self) -> Block {
        Block {
            header: self.header.clone(),
            transactions: vec![],
        }
    }
}

/// A set of operations that may update the Simperby node state.
///
/// `ProposeBlock` and `SubmitConsensusVote` are explicitly performed through the `SimperbyApi` trait, and the others
/// are done implicitly by the background task which is triggered by incoming p2p network messages.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum SimperbyOperation {
    ProposeBlock {
        block: BlockHeader,
        signature: TypedSignature<BlockHeader>,
    },
    SubmitConsensusVote {
        hash: Hash256,
        signature: TypedSignature<BlockHeader>,
    },
    ReceiveProposal {
        block: Block,
        round: Round,
        author_prevote: TypedSignature<(BlockHeader, Round)>,
    },
    ReceivePrevote {
        block_hash: Hash256,
        round: Round,
        author_prevote: TypedSignature<(BlockHeader, Round)>,
    },
    ReceivePrecommit {
        block_hash: Hash256,
        round: Round,
        author_prevote: TypedSignature<(BlockHeader, Round)>,
    },
    /// Used for sync.
    ReceiveFinalizedBlock {
        block: Block,
        finalization_proof: FinalizationProof,
    },
}

/// Errors that may occur while accessing the Simperby node.
#[derive(Error, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum SimperbyError {
    #[error("invalid block: {0}")]
    InvalidBlock(String),
    /// When the operation arguments are not valid.
    #[error("invalid operation: {0}")]
    InvalidOperation(String),
    #[error("storage error: {0}")]
    StorageError(simperby_kv_storage::Error),
    /// When the storage is corrupted.
    #[error("storage integrity error: {0}")]
    StorageIntegrityError(String),
    /// When the consensus safety is violated.
    #[error("consensus crisis: {0}")]
    Crisis(String),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct SimperbyOperationLog {
    pub description: String,
    pub timestamp: simperby_common::Timestamp,
    pub operation: SimperbyOperation,
    pub result: Option<SimperbyError>,
}

#[async_trait]
pub trait SimperbyApi {
    /// Gets the genesis info of the blockchain.
    fn get_genesis_info(&self) -> &GenesisInfo;

    /// Gets the current height of the blockchain.
    async fn get_height(&self) -> u64;

    /// Gets the finalized block for the given height.
    async fn get_block(&self, height: u64) -> Result<Block, SimperbyError>;

    /// Checks the given block as the next block to be added to the block of the given `height`.
    ///
    /// Fails if the block is invalid.
    async fn check_block(&self, block: Block, height: BlockHeight) -> Result<(), SimperbyError>;

    /// Reads the finalized state entry by the given key.
    async fn read_state(&self, key: String, height: BlockHeight) -> Result<Vec<u8>, SimperbyError>;

    /// Gets the current possible consensus voting options.
    async fn get_consensus_vote_options(&self) -> Result<Vec<ConsensusVoteItem>, SimperbyError>;

    /// Gets the current status of the ongoing consensus.
    ///
    /// This is essential because Simperby's consensus is 'interactive', which means that the validator
    /// has to understand what's going on and manually decide what to do regarding the consensus.
    ///
    /// TODO: define the type of the state.
    async fn get_consensus_status(&self) -> ();

    /// Gets the current status of the p2p network.
    ///
    /// Unlike the storage, the p2p network operations are done in the background, in other words,
    /// no [`SimperbyApi`] method directly accesses the p2p network.
    /// Thus we need a separate API to see its current status of it.
    ///
    /// TODO: define the type of the state.
    async fn get_network_status(&self) -> ();

    /// Gets the last `number` logs of attempts to execute `SimperbyOperation`s.
    async fn get_operation_log(&self, number: usize) -> Vec<SimperbyOperationLog>;

    /// Attempts to propose a block for this round. This may update the node state.
    ///
    /// It fails
    /// 1. with the same cause as `check_block`
    /// 2. if this node is not the current leader.
    /// 3. if this node has already proposed another block for this round.
    async fn propose_block(
        &self,
        block: Block,
        round: Round,
        prevote_signature: TypedSignature<(BlockHeader, Round)>,
    ) -> Result<(), SimperbyError>;

    /// Submits a vote for the given item, identified by its hash. This may update the node state.
    async fn submit_consensus_vote(
        &self,
        hash: Hash256,
        signature: Signature, // This is untyped because the signer doesn't know the source type.
    ) -> Result<(), SimperbyError>;
}

/// Initiates a live Simperby node.
///
/// - `state_storage` represents the current finalized state of the blockchain.
/// - `history_storage` is for storing a history of blockchain data, such as past blocks.
/// This is not essential to validate incoming blocks, but is used for sync protocol and queries.
pub async fn initiate_node(
    genesis_info: GenesisInfo,
    network: Box<dyn AuthorizedNetwork>,
    state_storage: Box<dyn KVStorage>,
    history_storage: Box<dyn KVStorage>,
) -> Result<impl SimperbyApi, anyhow::Error> {
    node::Node::new(genesis_info, network, state_storage, history_storage).await
}
