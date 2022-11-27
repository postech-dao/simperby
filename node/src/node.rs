use super::*;
use anyhow::anyhow;
use simperby_consensus::Consensus;
use simperby_network::primitives::{GossipNetwork, Storage};
use simperby_network::NetworkConfig;
use simperby_repository::raw::RawRepository;
use simperby_repository::DistributedRepository;

pub struct Node<N: GossipNetwork, S: Storage, R: RawRepository> {
    config: Config,
    repository: DistributedRepository<R>,
    governance: Governance<N, S>,
    #[allow(dead_code)]
    consensus: Consensus<N, S>,
}

async fn create_network_config(_config: &Config) -> Result<NetworkConfig> {
    unimplemented!()
}

#[async_trait]
impl<N: GossipNetwork, S: Storage, R: RawRepository> SimperbyApi for Node<N, S, R> {
    async fn genesis(&mut self) -> Result<()> {
        unimplemented!()
    }

    async fn initialize(&mut self) -> Result<()> {
        unimplemented!()
    }

    async fn sync(&mut self, _commmit: CommitHash) -> Result<()> {
        unimplemented!()
    }

    async fn clean(&mut self, _hard: bool) -> Result<()> {
        unimplemented!()
    }

    async fn create_block(&mut self) -> Result<()> {
        unimplemented!()
    }

    async fn create_agenda(&mut self) -> Result<()> {
        unimplemented!()
    }

    async fn create_extra_agenda_transaction(&mut self, _tx: ExtraAgendaTransaction) -> Result<()> {
        unimplemented!()
    }

    async fn vote(&mut self, agenda_commit: CommitHash) -> Result<()> {
        let valid_agendas = self.repository.get_agendas().await?;
        let agenda_hash = if let Some(x) = valid_agendas.iter().find(|(x, _)| *x == agenda_commit) {
            x.1
        } else {
            return Err(anyhow!(
                "the given commit hash {} is not one of the valid agendas",
                agenda_commit
            ));
        };
        self.governance
            .vote(
                &create_network_config(&self.config).await?,
                &[],
                agenda_hash,
                &self.config.private_key,
            )
            .await?;
        Ok(())
    }

    async fn veto_round(&mut self) -> Result<()> {
        unimplemented!()
    }

    async fn veto_block(&mut self, _block_commit: CommitHash) -> Result<()> {
        unimplemented!()
    }

    async fn show(&self, _commit: CommitHash) -> Result<CommitInfo> {
        todo!()
    }

    async fn run(self) -> Result<()> {
        unimplemented!()
    }

    async fn progress_for_consensus(&mut self) -> Result<String> {
        unimplemented!()
    }

    async fn get_consensus_status(&self) -> Result<ConsensusStatus> {
        unimplemented!()
    }

    async fn get_network_status(&self) -> Result<NetworkStatus> {
        unimplemented!()
    }

    async fn relay(self) -> Result<()> {
        unimplemented!()
    }

    async fn fetch(&mut self) -> Result<()> {
        unimplemented!()
    }
}
