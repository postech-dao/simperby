use serde::{Deserialize, Serialize};
use simperby_common::{
    crypto::{Hash256, PublicKey},
    ConsensusRound, PrivateKey, Timestamp, ToHash256, TypedSignature, VotingPower,
};
use simperby_network::{
    dms::{DistributedMessageSet as DMS, Message, MessageFilter},
    primitives::{GossipNetwork, Storage},
    *,
};
use std::collections::BTreeSet;
use std::sync::Arc;
use vetomint::*;

pub type Error = anyhow::Error;
const STATE_FILE_NAME: &str = "state.json";
pub type Nil = ();
const NIL_BLOCK_PROPOSAL_INDEX: BlockIdentifier = BlockIdentifier::MAX;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub consensus_state: ConsensusState,
    /// The set of messages that have been already updated to the Vetomint state machine.
    pub updated_message: BTreeSet<Hash256>,
    /// The set of the block hashes that have been verified.
    pub verified_block_hash: Vec<Hash256>,
    /// The validator set eligible for this block
    pub validator_set: Vec<(PublicKey, VotingPower)>,
    /// If this node is a particiapnt, the index of this node.
    pub this_node_index: Option<usize>,
    /// If true, this every operation on this instance will fail; the user must
    /// run `create()` to create a new instance.
    pub finalized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusMessage {
    Proposal(Hash256),
    NonNilPreVoted(
        ConsensusRound,
        /// The hash of the voted block
        Hash256,
    ),
    NonNilPreComitted(ConsensusRound, Hash256),
    NilPreVoted(ConsensusRound),
    NilPreComitted(ConsensusRound),
}

pub enum ProgressResult {
    Proposed(ConsensusRound, Hash256, Timestamp),
    NonNilPreVoted(ConsensusRound, Hash256, Timestamp),
    NonNilPreComitted(ConsensusRound, Hash256, Timestamp),
    NilPreVoted(ConsensusRound, Timestamp),
    NilPreComitted(ConsensusRound, Timestamp),
    Finalized(Hash256, Timestamp),
}

pub struct ConsensusMessageFilter {
    /// Note that it is even DESIRABLE to use a synchronous lock in the async context,
    /// if it is guaranteed that the lock is not held for a long time.
    verified_block_hash: Arc<parking_lot::RwLock<BTreeSet<Hash256>>>,
    validator_set: BTreeSet<PublicKey>,
}

impl MessageFilter for ConsensusMessageFilter {
    fn filter(&self, message: &Message) -> Result<(), String> {
        serde_json::from_str::<ConsensusMessage>(message.data()).map_err(|e| e.to_string())?;
        if !self.validator_set.contains(message.signature().signer()) {
            return Err("The signer is not in the validator set".to_string());
        }
        if self
            .verified_block_hash
            .read()
            .contains(&message.to_hash256())
        {
            Ok(())
        } else {
            Err("The block hash is not validated yet.".to_string())
        }
    }
}

pub struct Consensus<N: GossipNetwork, S: Storage> {
    dms: DMS<N, S>,
    /// A cache of the consensus state.
    state: State,
    /// The set of the block hashes that have been verified, shared by the message filter.
    ///
    /// Note that there is the exactly same copy in the `state`.
    verified_block_hash: Arc<parking_lot::RwLock<BTreeSet<Hash256>>>,
    /// (If participated) the private key of this node
    this_node_key: Option<PrivateKey>,
}

impl<N: GossipNetwork, S: Storage> Consensus<N, S> {
    pub async fn create(
        dms: DMS<N, S>,
        validator_set: &[(PublicKey, VotingPower)],
        this_node_key: Option<usize>,
        timestamp: Timestamp,
        consensus_params: ConsensusParams,
    ) -> Result<(), Error> {
        let this_node_index = this_node_key
            .map(|key| {
                validator_set
                    .iter()
                    .position(|(pk, _)| *pk == validator_set[key].0)
                    .ok_or_else(|| anyhow::anyhow!("The validator set does not contain this node."))
            })
            .transpose()?;
        let height_info = HeightInfo {
            validators: validator_set.iter().map(|(_, v)| *v).collect(),
            this_node_index,
            timestamp,
            consensus_params,
            initial_block_candidate: NIL_BLOCK_PROPOSAL_INDEX,
        };
        let state = State {
            consensus_state: ConsensusState::new(height_info),
            updated_message: BTreeSet::new(),
            verified_block_hash: vec![],
            validator_set: validator_set.to_vec(),
            finalized: false,
            this_node_index,
        };
        dms.get_storage()
            .write()
            .await
            .add_or_overwrite_file(STATE_FILE_NAME, serde_json::to_string(&state).unwrap())
            .await?;
        Ok(())
    }

    pub async fn new(mut dms: DMS<N, S>, this_node_key: Option<PrivateKey>) -> Result<Self, Error> {
        let verified_block_hash = Arc::new(parking_lot::RwLock::new(BTreeSet::new()));
        let state = dms
            .get_storage()
            .read()
            .await
            .read_file(STATE_FILE_NAME)
            .await?;
        let state: State = serde_json::from_str(&state)?;
        if let Some(index) = state.this_node_index {
            if state.validator_set[index].0
                != this_node_key
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("private key is required"))?
                    .public_key()
            {
                anyhow::bail!("the private key does not match");
            }
        }
        dms.set_filter(Arc::new(ConsensusMessageFilter {
            verified_block_hash: Arc::clone(&verified_block_hash),
            validator_set: state
                .validator_set
                .iter()
                .map(|(pk, _)| pk.clone())
                .collect(),
        }));

        Ok(Self {
            dms,
            state,
            verified_block_hash,
            this_node_key,
        })
    }

    pub async fn register_verified_block_hash(&mut self, hash: Hash256) -> Result<(), Error> {
        self.state.verified_block_hash.push(hash);
        self.verified_block_hash.write().insert(hash);
        self.dms
            .get_storage()
            .write()
            .await
            .add_or_overwrite_file(STATE_FILE_NAME, serde_json::to_string(&self.state).unwrap())
            .await?;
        Ok(())
    }

    pub async fn read(&self) -> Result<ConsensusState, Error> {
        Ok(self.state.consensus_state.clone())
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
        network_config: &NetworkConfig,
        known_peers: &[Peer],
        timestamp: Timestamp,
    ) -> Result<Vec<ProgressResult>, Error> {
        let messages = self.dms.read_messages().await?;
        let messages = messages
            .into_iter()
            .filter(|m| !self.state.updated_message.contains(&m.to_hash256()))
            .collect::<Vec<_>>();
        let mut final_result = Vec::new();
        for message in messages {
            let consensus_message = serde_json::from_str::<ConsensusMessage>(message.data())
                .expect("this must be already verified by the message filter");
            let signer = self
                .state
                .validator_set
                .iter()
                .position(|(pk, _)| pk == message.signature().signer())
                .expect("this must be already verified by the message filter");
            let event = match consensus_message {
                ConsensusMessage::NonNilPreVoted(round, block_hash) => {
                    let index = self
                        .state
                        .verified_block_hash
                        .iter()
                        .position(|h| h == &block_hash)
                        .expect("this must be already verified by the message filter");
                    ConsensusEvent::Prevote {
                        proposal: index,
                        signer,
                        round: round as usize,
                        time: timestamp,
                    }
                }
                _ => unimplemented!(),
            };
            // Not directly commit the change in case of possible dms errors.
            let mut consensus_state_copy = self.state.consensus_state.clone();
            let result = consensus_state_copy.progress(event);
            if let Some(result) = result {
                for response in result {
                    match response {
                        ConsensusResponse::BroadcastNilPrevote { round } => {
                            if let Some(private_key) = self.this_node_key.as_ref() {
                                let message = ConsensusMessage::NilPreVoted(round as u64);
                                let message = serde_json::to_string(&message).unwrap();
                                let signature = TypedSignature::sign(&message, private_key)
                                    .expect("private key already verified");
                                self.dms
                                    .add_message(
                                        network_config,
                                        known_peers,
                                        Message::new(message, signature)
                                            .expect("signature just created"),
                                    )
                                    .await?
                            }
                            final_result.push(ProgressResult::NilPreVoted(round as u64, timestamp));
                        }
                        _ => unimplemented!(),
                    }
                }
            }
            self.state.consensus_state = consensus_state_copy;
        }
        Ok(final_result)
    }

    pub async fn fetch(
        &mut self,
        network_config: &NetworkConfig,
        known_peers: &[Peer],
    ) -> Result<(), Error> {
        self.dms.fetch(network_config, known_peers).await?;
        Ok(())
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
