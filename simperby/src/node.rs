mod storage;

use super::*;
use simperby_kv_storage::KVStorage;
use simperby_network::AuthorizedNetwork;
use std::sync::Arc;
use storage::*;
use tokio::sync::RwLock;
use vetomint::Round;

pub struct Node {
    state: Arc<RwLock<NodeState>>,
    network_task: tokio::task::JoinHandle<()>,
    genesis_info: GenesisInfo,
}

impl Node {
    pub(crate) async fn new(
        genesis_info: GenesisInfo,
        network: Box<dyn AuthorizedNetwork>,
        state_storage: Box<dyn KVStorage>,
        history_storage: Box<dyn KVStorage>,
    ) -> Result<Self, anyhow::Error> {
        let last_header = genesis_info.header.clone();
        let state_ = Arc::new(RwLock::new(NodeState {
            state_storage,
            history_storage: HistoryStorage::new(history_storage, genesis_info.clone()).await?,
            last_header,
        }));
        Ok(Node {
            state: Arc::clone(&state_),
            network_task: tokio::task::spawn(async move {
                run_network_task(network, state_).await;
            }),
            genesis_info,
        })
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        self.network_task.abort();
    }
}

#[async_trait]
impl SimperbyApi for Node {
    fn get_genesis_info(&self) -> &GenesisInfo {
        &self.genesis_info
    }

    async fn get_height(&self) -> u64 {
        self.state.read().await.last_header.height
    }

    async fn get_block(&self, height: BlockHeight) -> Result<Block, SimperbyError> {
        let state = self.state.read().await;
        if state.last_header.height < height {
            return Err(SimperbyError::InvalidOperation(format!(
                "height {} is greater than the current height {}",
                height, state.last_header.height
            )));
        }

        state
            .history_storage
            .get_block(height)
            .await
            .map_err(|e| e.into())
    }

    async fn check_block(&self, _block: Block, _height: BlockHeight) -> Result<(), SimperbyError> {
        unimplemented!()
    }

    async fn read_state(&self, key: String, height: u64) -> Result<Vec<u8>, SimperbyError> {
        let state = self.state.read().await;
        assert_eq!(
            height, state.last_header.height,
            "currently state query is supported only for the last block"
        );
        state
            .state_storage
            .get(Hash256::hash(key))
            .await
            .map_err(SimperbyError::StorageError)
    }

    async fn get_consensus_vote_options(&self) -> Result<Vec<ConsensusVoteItem>, SimperbyError> {
        unimplemented!()
    }

    async fn get_consensus_status(&self) -> () {
        unimplemented!()
    }

    async fn get_network_status(&self) -> () {
        unimplemented!()
    }

    /// Gets the `number`-last logs of attempts to execute `SimperbyOperation`s.
    async fn get_operation_log(&self, _number: usize) -> Vec<SimperbyOperationLog> {
        unimplemented!()
    }

    async fn propose_block(
        &self,
        block: Block,
        round: Round,
        prevote_signature: TypedSignature<(BlockHeader, Round)>,
    ) -> Result<(), SimperbyError> {
        self.state
            .write()
            .await
            .submit_block_proposal(block, round, prevote_signature)
    }

    async fn submit_consensus_vote(
        &self,
        _hash: Hash256,
        _signature: TypedSignature<Hash256>,
    ) -> Result<(), SimperbyError> {
        unimplemented!()
    }
}

async fn run_network_task(
    _network: Box<dyn AuthorizedNetwork>,
    _node_state: Arc<RwLock<NodeState>>,
) {
    unimplemented!()
}

/// The node state machine.
///
/// Both `SimperbyApi` and `run_network_task()` can concurrently access to this state
/// and it will be synchronized by `RwLock`.
struct NodeState {
    history_storage: HistoryStorage,
    state_storage: Box<dyn KVStorage>,
    // TODO: add `consensus: vetomint::ConsensusState,`
    /// A cache of the latest finalized block (which is also in the history storage)
    last_header: BlockHeader,
}

impl NodeState {
    fn submit_block_proposal(
        &mut self,
        _block: Block,
        _round: Round,
        _prevote_signature: TypedSignature<(BlockHeader, Round)>,
    ) -> Result<(), SimperbyError> {
        unimplemented!()
    }

    fn submit_vote_favor(&mut self, _hash: Hash256, _signature: TypedSignature<Hash256>) {
        unimplemented!()
    }

    fn receive_proposal(
        &mut self,
        _block: Block,
        _round: Round,
        _author_prevote: TypedSignature<(Hash256, Round)>,
    ) -> Result<(), SimperbyError> {
        unimplemented!()
    }

    /// If `block_hash` is `zero()` then it represents the nill vote.
    fn receive_prevote(
        &mut self,
        _block_hash: Hash256,
        _round: Round,
        _signature: TypedSignature<(Hash256, Round)>,
    ) -> Result<(), SimperbyError> {
        unimplemented!()
    }

    /// If `block_hash` is `zero()` then it represents the nill vote.
    fn receive_precommit(
        &mut self,
        _block_hash: Hash256,
        _round: vetomint::Round,
        _signature: TypedSignature<(Hash256, Round)>,
    ) -> Result<(), SimperbyError> {
        unimplemented!()
    }

    /// For sync.
    fn receive_finalized_block(
        &mut self,
        _block: Block,
        _finalization_proof: Vec<(PublicKey, TypedSignature<BlockHeader>)>,
    ) -> Result<(), SimperbyError> {
        unimplemented!()
    }
}
