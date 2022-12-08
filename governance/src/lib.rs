use serde::{Deserialize, Serialize};
use simperby_common::*;
use simperby_network::{
    dms::{DistributedMessageSet as DMS, Message},
    primitives::{GossipNetwork, Storage},
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
    pub this_node_key: Option<PrivateKey>,
}

impl<N: GossipNetwork, S: Storage> Governance<N, S> {
    pub async fn create(_dms: DMS<N, S>, _height: BlockHeight) -> Result<(), Error> {
        Ok(())
    }

    pub async fn open(dms: DMS<N, S>, this_node_key: Option<PrivateKey>) -> Result<Self, Error> {
        Ok(Self { dms, this_node_key })
    }

    pub async fn read(&self) -> Result<GovernanceStatus, Error> {
        let messages = self.dms.read_messages().await?;
        let votes = messages
            .iter()
            .map(|message| {
                let vote: Vote = serde_json::from_str(message.data()).unwrap();
                (vote.agenda_hash, vote.voter)
            })
            .fold(HashMap::new(), |mut votes, (agenda_hash, voter)| {
                votes
                    .entry(agenda_hash)
                    .or_insert_with(HashSet::new)
                    .insert(voter);
                votes
            });
        let height = self.dms.read_height().await?;
        let status = GovernanceStatus { votes, height };
        Ok(status)
    }

    pub async fn vote(&mut self, agenda_hash: Hash256) -> Result<(), Error> {
        let data = serde_json::to_string(&Vote {
            agenda_hash,
            voter: self.this_node_key.as_ref().unwrap().public_key(),
            signature: Signature::sign(agenda_hash, self.this_node_key.as_ref().unwrap())?,
        })
        .unwrap();
        let message = Message::new(
            data.clone(),
            TypedSignature::sign(&data, self.this_node_key.as_ref().unwrap())?,
        )?;

        self.dms.add_message(message).await?;
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

    pub async fn fetch(&mut self) -> Result<(), Error> {
        self.dms.fetch().await?;
        Ok(())
    }

    /// Serves the governance protocol indefinitely.
    pub async fn serve(self) -> Result<tokio::task::JoinHandle<Result<(), Error>>, Error> {
        unimplemented!()
    }
}
