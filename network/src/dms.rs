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
