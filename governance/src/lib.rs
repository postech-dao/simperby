use serde::{Deserialize, Serialize};
use simperby_common::*;
use simperby_network::{
    dms::{DistributedMessageSet as DMS, Message},
    primitives::{GossipNetwork, Storage},
};
use std::collections::HashMap;

pub type Error = eyre::Error;

pub fn generate_dms_key(header: &BlockHeader) -> String {
    format!(
        "governance-{}-{}",
        header.height,
        &header.to_hash256().to_string()[0..8]
    )
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceStatus {
    /// Agenda hashes and their voters.
    pub votes: HashMap<Hash256, HashMap<PublicKey, Signature>>,
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
    /// TODO: this must take the eligible governance set for this height.
    pub async fn new(dms: DMS<N, S>, this_node_key: Option<PrivateKey>) -> Result<Self, Error> {
        Ok(Self { dms, this_node_key })
    }

    pub async fn read(&self) -> Result<GovernanceStatus, Error> {
        let messages = self.dms.read_messages().await?;
        let votes = messages
            .iter()
            .map(|message| {
                let vote: Vote = serde_json::from_str(message.data()).unwrap();
                (vote.agenda_hash, vote.voter, vote.signature)
            })
            .fold(
                HashMap::<_, HashMap<_, Signature>>::new(),
                |mut votes, (agenda_hash, voter, signature)| {
                    (*votes.entry(agenda_hash).or_default()).insert(voter, signature);
                    votes
                },
            );
        let status = GovernanceStatus { votes };
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

    pub async fn fetch(&mut self) -> Result<(), Error> {
        self.dms.fetch().await?;
        Ok(())
    }

    /// Serves the governance protocol indefinitely.
    ///
    /// TODO: currently it just returns itself after the given time.
    pub async fn serve(self, time_in_ms: u64) -> Result<Self, Error> {
        let dms = self.dms;
        let dms = dms.serve(time_in_ms).await?;
        Ok(Self {
            dms,
            this_node_key: self.this_node_key,
        })
    }
}
