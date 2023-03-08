use serde::{Deserialize, Serialize};
use simperby_common::*;
use simperby_network::{dms::DistributedMessageSet as DMS, primitives::Storage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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

pub struct Governance<S: Storage> {
    dms: Arc<RwLock<DMS<S>>>,
    this_node_key: Option<PrivateKey>,
}

impl<S: Storage> Governance<S> {
    /// TODO: this must take the eligible governance set for this height.
    pub async fn new(
        dms: Arc<RwLock<DMS<S>>>,
        this_node_key: Option<PrivateKey>,
    ) -> Result<Self, Error> {
        Ok(Self { dms, this_node_key })
    }

    pub async fn read(&self) -> Result<GovernanceStatus, Error> {
        let messages = self.dms.read().await.read_messages().await?;
        let votes = messages
            .iter()
            .map(|message| {
                let vote: Vote = serde_spb::from_str(message.data()).unwrap();
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
        let message = serde_spb::to_string(&Vote {
            agenda_hash,
            voter: self.this_node_key.as_ref().unwrap().public_key(),
            signature: Signature::sign(agenda_hash, self.this_node_key.as_ref().unwrap())?,
        })
        .unwrap();
        self.dms.write().await.add_message(message).await?;
        Ok(())
    }

    pub async fn update(&mut self) -> Result<(), Error> {
        Ok(())
    }

    pub fn get_dms(&self) -> Arc<RwLock<DMS<S>>> {
        Arc::clone(&self.dms)
    }
}
