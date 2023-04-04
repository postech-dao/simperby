use serde::{Deserialize, Serialize};
use simperby_core::utils::get_timestamp;
use simperby_core::*;
use simperby_network::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type Error = eyre::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceStatus {
    /// Agenda hashes and their voters.
    pub votes: HashMap<Hash256, HashMap<PublicKey, Signature>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Vote {
    pub agenda_hash: Hash256,
}

impl ToHash256 for Vote {
    fn to_hash256(&self) -> Hash256 {
        self.agenda_hash
    }
}

impl DmsMessage for Vote {
    fn check(&self) -> Result<(), Error> {
        Ok(())
    }

    /// Agenda hash cryptographically contains the information of height. It's safe to ignore `dms_key`.
    fn commit(
        &self,
        _dms_key: &DmsKey,
        private_key: &PrivateKey,
    ) -> Result<MessageCommitmentProof, CryptoError>
    where
        Self: Sized,
    {
        Signature::sign(self.to_hash256(), private_key).map(|signature| MessageCommitmentProof {
            committer: private_key.public_key(),
            signature,
        })
    }

    fn verify_commitment(
        &self,
        proof: &MessageCommitmentProof,
        _dms_key: &DmsKey,
    ) -> Result<(), CryptoError> {
        proof.signature.verify(self.to_hash256(), &proof.committer)
    }
}

pub struct Governance {
    dms: Arc<RwLock<Dms<Vote>>>,
    fi: FinalizationInfo,
}

impl Governance {
    pub async fn new(dms: Arc<RwLock<Dms<Vote>>>, fi: FinalizationInfo) -> Result<Self, Error> {
        // TODO: this must set the DMS to accept messages only from
        // the eligible governance set for this height.
        Ok(Self { dms, fi })
    }

    pub async fn read(&self) -> Result<GovernanceStatus, Error> {
        let votes = self.dms.read().await.read_messages().await?;
        let mut result = HashMap::<Hash256, HashMap<PublicKey, Signature>>::default();
        for vote in votes {
            for committers in vote.committers {
                result
                    .entry(vote.message.to_hash256())
                    .or_default()
                    .insert(committers.committer, committers.signature);
            }
        }
        let status = GovernanceStatus { votes: result };
        Ok(status)
    }

    pub async fn get_eligible_agendas(&self) -> Result<Vec<(Hash256, AgendaProof)>, Error> {
        let governance_set = self
            .fi
            .reserved_state
            .get_governance_set()
            // TODO: handle integrity error
            .unwrap()
            .into_iter()
            .collect::<HashMap<_, _>>();
        let governance_state = self.read().await?;
        let votes: Vec<(Hash256, VotingPower)> = governance_state
            .votes
            .iter()
            .map(|(agenda, votes)| {
                (
                    *agenda,
                    votes
                        .keys()
                        .map(|voter| governance_set.get(voter).unwrap())
                        .sum(),
                )
            })
            .collect();
        let mut result = Vec::new();
        let total_voting_power = governance_set.values().sum::<VotingPower>();
        for (agenda, voted_power) in votes {
            if voted_power * 2 > total_voting_power {
                let proof: Vec<_> = governance_state.votes[&agenda]
                    .iter()
                    .map(|(k, s)| TypedSignature::<Agenda>::new(s.clone(), k.clone()))
                    .collect();
                result.push((
                    agenda,
                    AgendaProof {
                        height: self.fi.header.height + 1,
                        agenda_hash: agenda,
                        proof,
                        timestamp: get_timestamp(),
                    },
                ));
            }
        }
        Ok(result)
    }

    pub async fn vote(&mut self, agenda_hash: Hash256) -> Result<(), Error> {
        self.dms
            .write()
            .await
            .commit_message(&Vote { agenda_hash })
            .await?;
        Ok(())
    }

    pub async fn flush(&self) -> Result<(), Error> {
        Ok(())
    }

    pub async fn update(&mut self) -> Result<(), Error> {
        Ok(())
    }

    pub fn get_dms(&self) -> Arc<RwLock<Dms<Vote>>> {
        Arc::clone(&self.dms)
    }
}
