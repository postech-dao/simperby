use async_trait::async_trait;
use simperby_common::BlockHeight;
use simperby_network::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("network error: {0}")]
    Network(simperby_network::Error),
    #[error("unknown error: {0}")]
    Unknown(String),
}

#[async_trait]
pub trait DistributedRepository {
    async fn create(directory: &str, height: BlockHeight) -> Result<(), Error>;

    async fn fetch(
        directory: &str,
        network_config: NetworkConfig,
        known_peers: &[Peer],
    ) -> Result<(), Error>;

    /// Notifies there was a push for the given repository.
    async fn notify_push(
        directory: &str,
        network_config: NetworkConfig,
        known_peers: &[Peer],
    ) -> Result<(), Error>;

    /// Serves the distributed repository protocol indefinitely.
    async fn serve(
        directory: &str,
        network_config: &NetworkConfig,
        peers: SharedKnownPeers,
    ) -> Result<tokio::task::JoinHandle<Result<(), Error>>, Error>;
}
