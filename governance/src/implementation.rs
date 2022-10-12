use super::*;
use serde::{Deserialize, Serialize};
use simperby_network::{DistributedMessageSet, DistributedMessageSetImpl as DMS};

pub struct GovernanceImpl {
    dms: DMS,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Vote {
    pub agenda_hash: Hash256,
    pub voter: PublicKey,
    pub signature: Signature,
}

#[async_trait]
impl Governance for GovernanceImpl {
    async fn create(_directory: &str, _height: BlockHeight) -> Result<(), Error> {
        unimplemented!()
    }

    async fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        Ok(Self {
            dms: DMS::open(directory).await?,
        })
    }

    async fn read(&self) -> Result<GovernanceState, Error> {
        unimplemented!()
    }

    async fn vote(
        &mut self,
        network_config: &NetworkConfig,
        known_peers: &[Peer],
        agenda_hash: Hash256,
        private_key: &PrivateKey,
    ) -> Result<(), Error> {
        self.dms
            .add_message(
                network_config,
                known_peers,
                serde_json::to_vec(&Vote {
                    agenda_hash,
                    voter: private_key.public_key(),
                    signature: Signature::sign(agenda_hash, private_key)?,
                })
                .unwrap(),
            )
            .await?;
        Ok(())
    }

    /// Advances the block height, discarding all the votes.
    async fn advance(&mut self, _height_to_assert: BlockHeight) -> Result<(), Error> {
        unimplemented!()
    }

    async fn fetch(
        &mut self,
        _network_config: &NetworkConfig,
        _known_peers: &[Peer],
    ) -> Result<(), Error> {
        unimplemented!()
    }

    /// Serves the governance protocol indefinitely.
    async fn serve(
        self,
        _network_config: &NetworkConfig,
        _peers: SharedKnownPeers,
    ) -> Result<tokio::task::JoinHandle<Result<(), Error>>, Error> {
        unimplemented!()
    }
}
