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
pub struct GovernanceStatus {
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
        Ok(())
    }

    pub async fn open(dms: DMS<N, S>) -> Result<Self, Error> {
        Ok(Self { dms })
    }

    pub async fn read(&self) -> Result<GovernanceStatus, Error> {
        let messages = self.dms.read_messages().await?;
        let votes = messages
            .iter()
            .map(|message| {
                (
                    message.data().to_string().to_hash256(),
                    message.signature().signer().clone(),
                )
            })
            .fold(HashMap::new(), |mut votes, (data_hash, voter)| {
                votes
                    .entry(data_hash)
                    .or_insert_with(HashSet::new)
                    .insert(voter);
                votes
            });
        let height = self.dms.read_height().await?;
        let status = GovernanceStatus { votes, height };
        Ok(status)
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
    pub async fn advance(&mut self, height_to_assert: BlockHeight) -> Result<(), Error> {
        let height: BlockHeight = self.dms.read_height().await?;
        if height != height_to_assert {
            return Err(anyhow::anyhow!(
                "the height of the governance status is not the expected one: {} != {}",
                height,
                height_to_assert
            ));
        }
        self.dms.advance().await?;
        Ok(())
    }

    pub async fn fetch(
        &mut self,
        network_config: &NetworkConfig,
        known_peers: &[Peer],
    ) -> Result<(), Error> {
        self.dms.fetch(network_config, known_peers).await?;
        Ok(())
    }

    /// Serves the governance protocol indefinitely.
    pub async fn serve(
        self,
        network_config: &NetworkConfig,
        peers: SharedKnownPeers,
    ) -> Result<tokio::task::JoinHandle<Result<(), Error>>, Error> {
        const RPC_PORT: u16 = 123;
        let join_handle = self
            .dms
            .serve(network_config.clone(), RPC_PORT, peers)
            .await?;
        Ok(join_handle)
    }
}
