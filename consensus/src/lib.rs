mod state;

use eyre::eyre;
use serde::{Deserialize, Serialize};
use simperby_core::utils::get_timestamp;
use simperby_core::*;
use simperby_network::*;
use state::*;
use std::collections::BTreeSet;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type Error = eyre::Error;

pub use state::ConsensusMessage;
pub use vetomint::ConsensusParams;

const STATE_FILE_NAME: &str = "state.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressResult {
    Proposed(ConsensusRound, Hash256, Timestamp),
    NonNilPreVoted(ConsensusRound, Hash256, Timestamp),
    NonNilPreCommitted(ConsensusRound, Hash256, Timestamp),
    NilPreVoted(ConsensusRound, Timestamp),
    NilPreCommitted(ConsensusRound, Timestamp),
    Finalized(Hash256, Timestamp, FinalizationProof),
    ViolationReported(PublicKey, String, Timestamp),
}

/// The consensus module
pub struct Consensus {
    /// The distributed consensus message set.
    dms: Arc<RwLock<Dms<ConsensusMessage>>>,
    /// The local storage for the consensus state.
    state_storage: StorageImpl,
}

impl Consensus {
    /// Creates a consensus instance.
    ///
    /// It clears and re-initializes the DMS and the stroage
    /// if the block header is different from the last one.
    pub async fn new(
        dms: Arc<RwLock<Dms<ConsensusMessage>>>,
        state_storage: StorageImpl,
        block_header: BlockHeader,
        consensus_parameters: ConsensusParams,
        round_zero_timestamp: Timestamp,
        this_node_key: Option<PrivateKey>,
    ) -> Result<Self, Error> {
        let mut this = Self { dms, state_storage };
        // Prepare new state in case of storage reset.
        let new_state = State::new(
            &block_header,
            consensus_parameters,
            round_zero_timestamp,
            this_node_key.clone().unwrap(),
        )?;
        if let Ok(state) = this.read_state().await {
            if block_header != *state.block_header() {
                return Err(eyre!("different block header in the storage"));
            }
        } else {
            this.dms.write().await.clear().await?;
            this.state_storage.remove_all_files().await?;
            this.commit_state(&new_state).await?;
        };

        if this
            .dms
            .read()
            .await
            .get_config()
            .members
            .iter()
            .collect::<BTreeSet<_>>()
            != block_header
                .validator_set
                .iter()
                .map(|(pubkey, _)| pubkey)
                .collect::<BTreeSet<_>>()
        {
            return Err(eyre!("validator set does not match the DMS members"));
        }
        Ok(this)
    }

    pub async fn get_block_header(&self) -> Result<BlockHeader, Error> {
        let state = self.read_state().await?;
        Ok(state.block_header().clone())
    }

    /// Checks whether the consensus is finalized.
    pub async fn check_finalized(&self) -> Result<Option<FinalizationProof>, Error> {
        let state = self.read_state().await?;
        Ok(state.check_finalized())
    }

    pub async fn register_verified_block_hash(&mut self, block_hash: Hash256) -> Result<(), Error> {
        let mut state = self.read_state().await?;
        state.register_verified_block_hash(block_hash);
        self.commit_state(&state).await?;
        Ok(())
    }

    /// Makes a progress in the consensus process.
    pub async fn progress(&mut self, timestamp: Timestamp) -> Result<Vec<ProgressResult>, Error> {
        let mut state = self.read_state().await?;
        let result = state.progress(timestamp);
        self.commit_state(&state).await?;
        Ok(result)
    }

    pub async fn set_proposal_candidate(
        &mut self,
        block_hash: Hash256,
        timestamp: Timestamp,
    ) -> Result<(), Error> {
        let mut state = self.read_state().await?;
        state.set_proposal_candidate(block_hash, timestamp)?;
        self.commit_state(&state).await?;
        Ok(())
    }

    pub async fn veto_block(&mut self, block_hash: Hash256) -> Result<(), Error> {
        let mut state = self.read_state().await?;
        state.veto_block(block_hash);
        self.commit_state(&state).await?;
        Ok(())
    }

    pub async fn veto_round(
        &mut self,
        round: ConsensusRound,
        timestamp: Timestamp,
    ) -> Result<(), Error> {
        let mut state = self.read_state().await?;
        state.veto_round(round, timestamp);
        self.commit_state(&state).await?;
        Ok(())
    }

    pub fn get_dms(&self) -> Arc<RwLock<Dms<ConsensusMessage>>> {
        Arc::clone(&self.dms)
    }

    pub async fn flush(&mut self) -> Result<(), Error> {
        // TODO: filter unverified messages (due to the lack of the block verification)
        let mut state = self.read_state().await?;
        let messages = state.drain_messages_to_broadcast();
        for message in messages {
            self.dms.write().await.commit_message(&message).await?;
        }
        Ok(())
    }

    pub async fn update(&mut self) -> Result<(), Error> {
        let mut state = self.read_state().await?;
        let messages = self.dms.read().await.read_messages().await?;
        let mut result = Vec::new();
        for message in messages {
            for commitment in message.committers {
                result.push((
                    message.message.clone(),
                    commitment.committer,
                    commitment.signature,
                ));
            }
        }
        state.add_consensus_messages(result, get_timestamp());
        self.commit_state(&state).await?;
        Ok(())
    }
}

// Various private methods.
impl Consensus {
    async fn read_state(&self) -> Result<State, Error> {
        let raw_state = self.state_storage.read_file(STATE_FILE_NAME).await?;
        let state: State = serde_spb::from_slice(&hex::decode(raw_state)?)?;
        Ok(state)
    }

    async fn commit_state(&mut self, state: &State) -> Result<(), Error> {
        // We can't use json because of a non-string map
        let data = hex::encode(serde_spb::to_vec(state).unwrap());
        self.state_storage
            .add_or_overwrite_file(STATE_FILE_NAME, data)
            .await
            .map_err(|_| eyre!("failed to commit consensus state to the storage"))
    }
}
