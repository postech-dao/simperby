use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simperby_common::{
    crypto::{Hash256, PublicKey},
    BlockHeight,
};
use simperby_network::*;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("network error: {0}")]
    Network(simperby_network::Error),
    #[error("unknown error: {0}")]
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceState {
    /// Agenda hashes and their voters.
    pub votes: HashMap<Hash256, HashSet<PublicKey>>,
    pub height: BlockHeight,
}

#[async_trait]
pub trait Governance {
    async fn create(directory: &str, height: BlockHeight) -> Result<(), Error>;

    async fn read(directory: &str) -> Result<GovernanceState, Error>;

    async fn vote(
        directory: &str,
        network_config: NetworkConfig,
        known_peers: &[Peer],
        agenda_hash: Hash256,
        height_to_assert: BlockHeight,
    ) -> Result<(), Error>;

    /// Advances the block height, discarding all the votes.
    async fn advance(directory: &str, height_to_assert: BlockHeight) -> Result<(), Error>;

    async fn fetch(
        directory: &str,
        network_config: NetworkConfig,
        known_peers: &[Peer],
    ) -> Result<(), Error>;

    /// Serves the governance protocol indefinitely.
    async fn serve(
        directory: &str,
        network_config: &NetworkConfig,
    ) -> Result<tokio::task::JoinHandle<Result<(), Error>>, Error>;
}
