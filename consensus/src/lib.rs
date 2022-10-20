use serde::{Deserialize, Serialize};
use simperby_common::{
    crypto::{Hash256, PublicKey},
    BlockHeight, ConsensusRound, Timestamp, VotingPower,
};
use simperby_network::{
    dms::DistributedMessageSet as DMS,
    primitives::{P2PNetwork, Storage},
    *,
};
use std::collections::{HashMap, HashSet};

pub type Error = anyhow::Error;

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

pub struct Consensus<N: P2PNetwork, S: Storage> {
    pub dms: DMS<N, S>,
}

impl<N: P2PNetwork, S: Storage> Consensus<N, S> {
    pub async fn create(
        _dms: DMS<N, S>,
        _height: BlockHeight,
        _validator_set: &[(PublicKey, VotingPower)],
    ) -> Result<(), Error> {
        unimplemented!()
    }

    pub async fn new(_dms: DMS<N, S>) -> Result<Self, Error> {
        unimplemented!()
    }

    pub async fn read(&self) -> Result<ConsensusState, Error> {
        unimplemented!()
    }

    pub async fn veto_block(
        &mut self,
        _network_config: NetworkConfig,
        _known_peers: &[Peer],
        _block_hash: Hash256,
    ) -> Result<(), Error> {
        unimplemented!()
    }

    pub async fn set_proposal(&mut self, _block_hash: Hash256) -> Result<(), Error> {
        unimplemented!()
    }

    pub async fn veto_round(
        &mut self,
        _network_config: NetworkConfig,
        _known_peers: &[Peer],
        _round: ConsensusRound,
    ) -> Result<(), Error> {
        unimplemented!()
    }

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
    pub async fn progress(
        &mut self,
        _network_config: NetworkConfig,
        _known_peers: &[Peer],
    ) -> Result<Vec<ProgressResult>, Error> {
        unimplemented!()
    }

    pub async fn fetch(
        &mut self,
        _network_config: NetworkConfig,
        _known_peers: &[Peer],
    ) -> Result<(), Error> {
        unimplemented!()
    }

    /// Serves the consensus protocol indefinitely.
    ///
    /// 1. It does `DistributedMessageSet::serve()`.
    /// 2. It does `Consensus::progress()` continuously.
    pub async fn serve(
        self,
        _network_config: NetworkConfig,
        _peers: SharedKnownPeers,
    ) -> Result<
        (
            tokio::sync::mpsc::Receiver<ProgressResult>,
            tokio::task::JoinHandle<Result<(), Error>>,
        ),
        Error,
    > {
        unimplemented!()
    }
}
