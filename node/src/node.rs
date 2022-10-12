use super::*;
use anyhow::anyhow;
use simperby_consensus::Consensus;
use simperby_network::NetworkConfig;
use simperby_repository::raw::RawRepository;
use simperby_repository::DistributedRepository;

pub struct Node {
    config: Config,
}

async fn create_network_config(_config: &Config) -> Result<NetworkConfig> {
    unimplemented!()
}

#[async_trait]
impl<RR: RawRepository, R: DistributedRepository<RR>, C: Consensus, G: Governance>
    SimperbyApi<RR, R, C, G> for Node
{
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
        let repo = R::new(RR::open(&self.config.repository_directory).await?).await?;
        let valid_agendas = repo.get_agendas().await?;
        let agenda_hash = if let Some(x) = valid_agendas.iter().find(|(x, _)| *x == agenda_commit) {
            x.1
        } else {
            return Err(anyhow!(
                "the given commit hash {} is not one of the valid agendas",
                agenda_commit
            ));
        };
        let mut governance = G::open(&self.config.governance_directory).await?;
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
