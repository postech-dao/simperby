use serde::{Deserialize, Serialize};
use simperby_common::*;
use simperby_network::{
    dms::{DistributedMessageSet as DMS, Message},
    primitives::{GossipNetwork, Storage},
    NetworkConfig, Peer, SharedKnownPeers,
};
use std::collections::{HashMap, HashSet};

pub type Error = anyhow::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceState {
    /// Agenda hashes and their voters.
    pub votes: HashMap<Hash256, HashSet<PublicKey>>,
    pub height: BlockHeight,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Vote {
    pub agenda_hash: Hash256,
    pub voter: PublicKey,
    pub signature: Signature,
}

pub struct Governance<N: GossipNetwork, S: Storage> {
    pub dms: DMS<N, S>,
}

impl<N: GossipNetwork, S: Storage> Governance<N, S> {
    pub async fn create(_dms: DMS<N, S>, _height: BlockHeight) -> Result<(), Error> {
        unimplemented!()
    }

    pub async fn open(_dms: DMS<N, S>) -> Result<Self, Error> {
        unimplemented!()
    }

    pub async fn read(&self) -> Result<GovernanceState, Error> {
        unimplemented!()
    }

    pub async fn vote(
        &mut self,
        network_config: &NetworkConfig,
        known_peers: &[Peer],
        agenda_hash: Hash256,
        private_key: &PrivateKey,
    ) -> Result<(), Error> {
        let data = serde_json::to_string(&Vote {
            agenda_hash,
            voter: private_key.public_key(),
            signature: Signature::sign(agenda_hash, private_key)?,
        })
        .unwrap();
        let message = Message::new(
            data.clone(),
            TypedSignature::sign(&data, &network_config.private_key)?,
        )?;

        self.dms
            .add_message(network_config, known_peers, message)
            .await?;
        Ok(())
    }

    /// Advances the block height, discarding all the votes.
    pub async fn advance(&mut self, _height_to_assert: BlockHeight) -> Result<(), Error> {
        unimplemented!()
    }

    pub async fn fetch(
        &mut self,
        _network_config: &NetworkConfig,
        _known_peers: &[Peer],
    ) -> Result<(), Error> {
        unimplemented!()
    }

    /// Serves the governance protocol indefinitely.
    pub async fn serve(
        self,
        _network_config: &NetworkConfig,
        _peers: SharedKnownPeers,
    ) -> Result<tokio::task::JoinHandle<Result<(), Error>>, Error> {
        unimplemented!()
    }
}
