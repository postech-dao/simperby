use std::time::Duration;

use super::*;
use anyhow::anyhow;
use simperby_network::dms::{Config as DmsConfig, DistributedMessageSet};
use simperby_network::primitives::{GossipNetwork, Storage};
use simperby_network::NetworkConfig;
use simperby_repository::raw::RawRepository;
use simperby_repository::DistributedRepository;

pub struct Node<N: GossipNetwork, S: Storage, R: RawRepository> {
    config: Config,
    _marker1: std::marker::PhantomData<N>,
    _marker2: std::marker::PhantomData<S>,
    _marker3: std::marker::PhantomData<R>,
}

async fn create_network_config(_config: &Config) -> Result<NetworkConfig> {
    unimplemented!()
}

#[async_trait]
impl<N: GossipNetwork, S: Storage, R: RawRepository> SimperbyApi for Node<N, S, R> {
    async fn genesis(&self) -> Result<()> {
        unimplemented!()
    }

    async fn initialize(&self) -> Result<()> {
        unimplemented!()
    }

    async fn sync(&self, _commmit: CommitHash) -> Result<()> {
        unimplemented!()
    }

    async fn clean(&self, _hard: bool) -> Result<()> {
        unimplemented!()
    }

    async fn create_block(&self) -> Result<()> {
        unimplemented!()
    }

    async fn create_agenda(&self) -> Result<()> {
        unimplemented!()
    }

    async fn create_extra_agenda_transaction(&self, _tx: ExtraAgendaTransaction) -> Result<()> {
        unimplemented!()
    }

    async fn vote(&self, agenda_commit: CommitHash) -> Result<()> {
        let repo =
            DistributedRepository::new(R::open(&self.config.repository_directory).await?).await?;
        let valid_agendas = repo.get_agendas().await?;
        let agenda_hash = if let Some(x) = valid_agendas.iter().find(|(x, _)| *x == agenda_commit) {
            x.1
        } else {
            return Err(anyhow!(
                "the given commit hash {} is not one of the valid agendas",
                agenda_commit
            ));
        };
        let governance_dms =
            DistributedMessageSet::open(S::open(&self.config.governance_directory).await?).await?;
        let mut governance = Governance::<N, S>::open(governance_dms).await?;
        governance
            .vote(
                &create_network_config(&self.config).await?,
                &[],
                agenda_hash,
                &self.config.private_key,
            )
            .await?;
        Ok(())
    }

    async fn veto_round(&self) -> Result<()> {
        unimplemented!()
    }

    async fn veto_block(&self, _block_commit: CommitHash) -> Result<()> {
        unimplemented!()
    }

    async fn run(&self) -> Result<()> {
        unimplemented!()
    }

    async fn progress_for_consensus(&self) -> Result<String> {
        unimplemented!()
    }

    async fn get_consensus_status(&self) -> Result<ConsensusStatus> {
        unimplemented!()
    }

    async fn get_network_status(&self) -> Result<NetworkStatus> {
        unimplemented!()
    }

    async fn relay(&self) -> Result<()> {
        unimplemented!()
    }

    async fn fetch(&self) -> Result<()> {
        unimplemented!()
    }

    async fn notify_git_push(&self) -> Result<String> {
        unimplemented!()
    }
}
