use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use simperby_common::{
    crypto::{Hash256, PublicKey},
    BlockHeight, ConsensusRound, Timestamp, VotingPower,
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
pub struct ConsensusState {
    pub non_nil_votes: HashMap<Hash256, HashSet<PublicKey>>,
    pub nil_votes: HashMap<Hash256, HashSet<PublicKey>>,
    pub height: BlockHeight,
    pub round: ConsensusRound,
}

pub enum ProgressResult {
    Proposed(ConsensusRound, Hash256, Timestamp),
    NonNilPreVoted(ConsensusRound, Hash256, Timestamp),
    NonNilPreComitted(ConsensusRound, Hash256, Timestamp),
    NilPreVoted(ConsensusRound, Timestamp),
    NilPreComitted(ConsensusRound, Timestamp),
    Finalized(Timestamp),
}

#[async_trait]
pub trait Consensus {
    async fn create(
        directory: &str,
        height: BlockHeight,
        validator_set: &[(PublicKey, VotingPower)],
    ) -> Result<(), Error>;

    async fn read(directory: &str) -> Result<ConsensusState, Error>;

    async fn veto_block(
        directory: &str,
        network_config: NetworkConfig,
        known_peers: &[Peer],
        block_hash: Hash256,
        height_to_assert: BlockHeight,
    ) -> Result<(), Error>;

    async fn veto_round(
        directory: &str,
        network_config: NetworkConfig,
        known_peers: &[Peer],
        round: ConsensusRound,
        height_to_assert: BlockHeight,
    ) -> Result<(), Error>;

    /// Makes a progress in the consensus process.
    /// It might
    ///
    /// 1. broadcast a proposal.
    /// 2. broadcast a pre-vote.
    /// 3. broadcast a pre-commit.
    /// 4. finalize the block and advance the height.
    ///
    /// For the case 4, it will clear the storage and will leave the finalization proof
    /// of the previous (just finalized) block.
    async fn progress(
        directory: &str,
        network_config: NetworkConfig,
        known_peers: &[Peer],
        height_to_assert: BlockHeight,
    ) -> Result<Vec<ProgressResult>, Error>;

    async fn fetch(
        directory: &str,
        network_config: NetworkConfig,
        known_peers: &[Peer],
    ) -> Result<(), Error>;

    /// Serves the consensus protocol indefinitely.
    ///
    /// 1. It does `DistributedMessageSet::serve()`.
    /// 2. It does `Consensus::progress()` continuously.
    async fn serve(
        directory: &str,
        network_config: NetworkConfig,
        peers: SharedKnownPeers,
    ) -> Result<
        (
            tokio::sync::mpsc::Receiver<ProgressResult>,
            tokio::task::JoinHandle<Result<(), Error>>,
        ),
        Error,
    >;
}
