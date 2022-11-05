#[cfg(feature = "full")]
mod behaviour;
#[cfg(feature = "full")]
mod primitive;
#[cfg(test)]
#[cfg(feature = "full")]
mod tests;
#[cfg(feature = "full")]
mod utils;

use super::*;
use async_trait::async_trait;

pub struct PeerDiscoveryImpl {}

#[async_trait]
impl PeerDiscovery for PeerDiscoveryImpl {
    async fn create(_storage_directory: &str) -> Result<(), Error> {
        unimplemented!();
    }

    async fn serve(
        _storage_directory: &str,
        _network_config: &NetworkConfig,
    ) -> Result<(SharedKnownPeers, tokio::task::JoinHandle<Result<(), Error>>), Error> {
        unimplemented!();
    }

    async fn read_known_peers(_storage_directory: &str) -> Result<Vec<Peer>, Error> {
        unimplemented!();
    }
}
