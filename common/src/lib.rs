pub mod types;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use types::*;

/// TODO: Provide error types.
///
/// Note: This trait is quite subject to change.
#[async_trait]
pub trait AuthorizedNetwork {
    /// Joins the network with an authorized identity.
    async fn new(
        public_key: PublicKey,
        private_key: PrivateKey,
        known_ids: Vec<PublicKey>,
        known_peers: Vec<std::net::Ipv4Addr>,
        network_id: String,
    ) -> Result<Self, String>
    where
        Self: Sized;
    /// Broadcasts a message to the network, after signed by the key given to this instance.
    async fn broadcast(&self, message: &[u8]) -> Result<(), String>;
    /// Creates a receiver for every message broadcasted to the network, except the one sent by this instance.
    async fn create_recv_queue(&self) -> Result<mpsc::Receiver<Vec<u8>>, ()>;
    /// Provides an estimated lists of live nodes that are eligible and identified by their public keys.
    async fn get_live_list(&self) -> Result<Vec<PublicKey>, ()>;
}

/// TODO: Provide error types.
///
/// Note: This trait is quite subject to change.
#[async_trait]
pub trait UnauthorizedNetwork {
    /// Joins the network with an authorized identity.
    async fn new(known_peers: Vec<std::net::Ipv4Addr>, network_id: String) -> Result<Self, String>
    where
        Self: Sized;
    /// Broadcasts a message to the network, after signed by the key given to this instance.
    async fn broadcast(&self, message: &[u8]) -> Result<(), String>;
    /// Creates a receiver for every message broadcasted to the network, except the one sent by this instance.
    async fn create_recv_queue(&self) -> Result<mpsc::Receiver<Vec<u8>>, ()>;
    /// Provides an estimated lists of live nodes identified by their IP addresses
    async fn get_live_list(&self) -> Result<Vec<std::net::Ipv4Addr>, ()>;
}

/// TODO: Provide error types.
///
/// Note: This trait is quite subject to change.
#[async_trait]
pub trait KVStore {
    /// Creates an empty store with the path to newly create.
    async fn new(path: &str) -> Result<Self, ()>
    where
        Self: Sized;
    /// Open an existing store with the path given.
    async fn open(path: &str) -> Result<Self, ()>
    where
        Self: Sized;
    /// Records the current state to the persistent storage.
    async fn commit_checkpoint(&mut self) -> Result<(), ()>;
    /// Reverts all the changes made since the last checkpoint.
    async fn reset_to_latest_checkpoint(&mut self) -> Result<(), ()>;
    /// Inserts a key-value pair into the store. If exists, it will be overwritten.
    async fn insert(&mut self, key: Hash256, value: &[u8]) -> Result<(), ()>;
    /// Remove a key-value pair from the store. If not exists, it will fail.
    async fn remove(&mut self, key: Hash256) -> Result<(), ()>;
    /// Retrieves the value associated with the key. If not exists, it will return `None`.
    async fn get(&self, key: Hash256) -> Result<Option<Vec<u8>>, ()>;
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Header {
    /// The author of this block.
    pub author: PublicKey,
    /// The signature of the previous block.
    pub prev_signature: Vec<Signature>,
    /// The hash of the previous block.
    pub previous_hash: Hash256,
    /// The height of this block.
    pub height: u64,
    /// The timestamp of this block.
    pub timestamp: u64,
    /// The Merkle root of transactions.
    pub tx_merkle_root: Hash256,
    /// The Merkle root of the state.
    pub state_merkle_root: Hash256,
    /// The hash of the set of validator & vote weight for the next block.
    pub validator_set_hash: Hash256,
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
    Noop,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Transaction {
    /// The siganture of this transaction.
    pub signature: Signature,
    /// The instruction to perform on the blockchain state.
    pub state_transition: StateTransition,
    /// An optional field to store data, which is not part of the state but still useful as it can be verified with the Merkle root.
    pub data: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Block {
    pub header: Header,
    pub transactions: Vec<Transaction>,
}

/// A state transition function.
#[async_trait]
pub trait BlockExecutor {
    async fn execute(&self, store: &mut dyn KVStore, transaction: Transaction) -> Result<(), ()>;
}

/// A BFT consensus engine.
#[async_trait]
pub trait Consensus {
    /// Peforms an one-block consensus.
    ///
    /// This method finishes when the next block is finalized.
    async fn progress(
        &mut self,
        block_to_propose: Option<Block>,
        last_finalized_header: Header,
        network: Arc<dyn AuthorizedNetwork>,
        executor: Box<dyn BlockExecutor>,
        store: Box<dyn KVStore>,
    ) -> Result<Block, ()>;
}
