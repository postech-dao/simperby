use crate::{primitives::*, *};
use async_trait::async_trait;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct DummyGossipNetwork;

#[async_trait]
impl GossipNetwork for DummyGossipNetwork {
    async fn broadcast(
        _config: &NetworkConfig,
        _known_peers: &[Peer],
        _message: Vec<u8>,
    ) -> Result<(), Error> {
        Ok(())
    }

    async fn serve(
        _config: NetworkConfig,
        _peers: SharedKnownPeers,
    ) -> Result<
        (
            mpsc::Receiver<Vec<u8>>,
            tokio::task::JoinHandle<Result<(), Error>>,
        ),
        Error,
    > {
        let (send, recv) = mpsc::channel(1);
        let task = tokio::spawn(async move {
            let _send = send;
            tokio::time::sleep(std::time::Duration::MAX).await;
            Ok(())
        });
        Ok((recv, task))
    }
}
