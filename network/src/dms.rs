use super::*;
use async_trait::async_trait;
use simperby_common::{crypto::*, BlockHeight};

/// The interface that will be wrapped into an HTTP RPC server for the peers.
#[async_trait]
pub trait DistributedMessageSetRpcInterface {
    /// Returns the messages except `knowns`. If the height is different, it returns `Err(height)`.
    async fn get_message(
        &self,
        height: BlockHeight,
        knowns: Vec<Hash256>,
    ) -> Result<Vec<Message>, BlockHeight>;
}

pub struct DistributedMessageSetImpl {}

#[async_trait]
impl DistributedMessageSet for DistributedMessageSetImpl {
    async fn create(_storage_directory: &str, _height: u64) -> Result<(), Error> {
        unimplemented!()
    }

    async fn open(_storage_directory: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        unimplemented!()
    }

    async fn fetch(
        &mut self,
        _network_config: &NetworkConfig,
        _known_peers: &[Peer],
    ) -> Result<(), Error> {
        unimplemented!()
    }

    async fn add_message(
        &mut self,
        _network_config: &NetworkConfig,
        _known_peers: &[Peer],
        _message: Vec<u8>,
    ) -> Result<(), Error> {
        unimplemented!()
    }

    async fn read_messages(&self) -> Result<(BlockHeight, Vec<Message>), Error> {
        unimplemented!()
    }

    async fn read_height(&self) -> Result<BlockHeight, Error> {
        unimplemented!()
    }

    async fn advance(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    async fn serve(
        self,
        _network_config: &NetworkConfig,
        _peers: SharedKnownPeers,
    ) -> Result<tokio::task::JoinHandle<Result<(), Error>>, Error> {
        unimplemented!()
    }
}
