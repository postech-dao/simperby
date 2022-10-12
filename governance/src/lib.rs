pub mod implementation;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simperby_common::*;
use simperby_network::{NetworkConfig, Peer, SharedKnownPeers};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("network error: {0}")]
    Network(simperby_network::Error),
    #[error("crypto error: {0}")]
    Crypto(CryptoError),
    #[error("unknown error: {0}")]
    Unknown(String),
}

impl From<simperby_network::Error> for Error {
    fn from(e: simperby_network::Error) -> Self {
        Error::Network(e)
    }
}

impl From<CryptoError> for Error {
    fn from(e: CryptoError) -> Self {
        Error::Crypto(e)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceState {
    /// Agenda hashes and their voters.
    pub votes: HashMap<Hash256, HashSet<PublicKey>>,
    pub height: BlockHeight,
}

#[async_trait]
pub trait Governance: Send + Sync {
    async fn create(directory: &str, height: BlockHeight) -> Result<(), Error>;

    async fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized;

    async fn read(&self) -> Result<GovernanceState, Error>;

    async fn vote(
        &mut self,
        network_config: &NetworkConfig,
        known_peers: &[Peer],
        agenda_hash: Hash256,
        private_key: &PrivateKey,
    ) -> Result<(), Error>;

    /// Advances the block height, discarding all the votes.
    async fn advance(&mut self, height_to_assert: BlockHeight) -> Result<(), Error>;

    async fn fetch(
        &mut self,
        network_config: &NetworkConfig,
        known_peers: &[Peer],
    ) -> Result<(), Error>;

    /// Serves the governance protocol indefinitely.
    async fn serve(
        self,
        network_config: &NetworkConfig,
        peers: SharedKnownPeers,
    ) -> Result<tokio::task::JoinHandle<Result<(), Error>>, Error>;
}
