mod storage;
pub mod types;

use eyre::eyre;
use eyre::Result;
use simperby_consensus::*;
use simperby_core::utils::get_timestamp;
use simperby_core::*;
use simperby_governance::*;
use simperby_network::*;
use simperby_repository::raw::RawRepository;
use simperby_repository::*;
use std::sync::Arc;
use tokio::sync::RwLock;

pub use crate::types::*;

/// A client for a single height.
struct ClientInner {
    config: types::Config,
    auth: Auth,
    path: String,
    repository: DistributedRepository,
    governance: Governance,
    consensus: Consensus,
}

/// An instance of Simperby client (a.k.a. a 'node').
pub struct Client {
    inner: Option<ClientInner>,
}

impl Client {
    pub async fn genesis(path: &str) -> Result<()> {
        let repository = RawRepository::open(path).await?;
        DistributedRepository::genesis(repository).await?;
        Ok(())
    }

    pub async fn init(path: &str, config: types::Config) -> Result<()> {
        storage::init(path, config).await?;
        Ok(())
    }

    pub async fn open(path: &str, config: types::Config, auth: Auth) -> Result<Self> {
        let (governance_dms, consensus_dms, consensus_state, repository) =
            storage::open(path, config.clone(), auth.clone()).await?;
        let lfi = repository.read_last_finalization_info().await?;

        Ok(Self {
            inner: Some(ClientInner {
                config,
                auth: auth.clone(),
                path: path.to_string(),
                repository,
                governance: Governance::new(Arc::new(RwLock::new(governance_dms)), lfi.clone())
                    .await?,
                consensus: Consensus::new(
                    Arc::new(RwLock::new(consensus_dms)),
                    consensus_state,
                    lfi.header,
                    ConsensusParams {
                        timeout_ms: 10000000,
                        repeat_round_for_first_leader: 100,
                    },
                    get_timestamp(),
                    Some(auth.private_key),
                )
                .await?,
            }),
        })
    }

    pub fn config(&self) -> &types::Config {
        &self.inner.as_ref().unwrap().config
    }

    pub fn auth(&self) -> &Auth {
        &self.inner.as_ref().unwrap().auth
    }

    pub async fn clean(&mut self, _hard: bool) -> Result<()> {
        todo!()
    }

    pub fn repository(&self) -> &DistributedRepository {
        &self.inner.as_ref().unwrap().repository
    }

    pub fn repository_mut(&mut self) -> &mut DistributedRepository {
        &mut self.inner.as_mut().unwrap().repository
    }

    /// Makes a progress for the consensus, returning the result.
    ///
    /// TODO: it has to consume the object if finalized.
    pub async fn progress_for_consensus(&mut self) -> Result<String> {
        let mut this = self.inner.take().unwrap();
        let result = this.consensus.progress(get_timestamp()).await?;
        let report = format!("{result:?}");
        for result in result {
            if let ProgressResult::Finalized(Finalization {
                block_hash, proof, ..
            }) = result
            {
                let commit_hash = this
                    .repository
                    .read_blocks()
                    .await?
                    .iter()
                    .find(|(_, h)| *h == block_hash)
                    .ok_or_else(|| eyre::eyre!("finalized block can't be found in repository"))?
                    .0;
                this.repository.finalize(commit_hash, proof).await?;
                let path = this.path.clone();
                let config = this.config.clone();
                let auth = this.auth.clone();
                drop(this);
                storage::clear(&path).await?;
                storage::init(&path, config.clone()).await?;
                self.inner = Some(Self::open(&path, config, auth).await?.inner.unwrap());
                return Ok(report);
            }
        }
        self.inner = Some(this);
        Ok(report)
    }

    pub async fn vote(&mut self, agenda_commit: CommitHash) -> Result<()> {
        let this = self.inner.as_mut().unwrap();
        let agendas = this.repository.read_agendas().await?;
        let agenda_hash = if let Some(x) = agendas.iter().find(|(x, _)| *x == agenda_commit) {
            x.1
        } else {
            return Err(eyre!(
                "the given commit hash {} is not one of the valid agendas",
                agenda_commit
            ));
        };
        this.repository.vote(agenda_commit).await?;
        this.governance.vote(agenda_hash).await?;
        Ok(())
    }

    /// Vetoes the current round.
    pub async fn veto_round(&mut self) -> Result<()> {
        unimplemented!()
    }

    /// Vetoes the given block.
    pub async fn veto_block(&mut self, _block_commit: CommitHash) -> Result<()> {
        unimplemented!()
    }

    /// Shows information about the given commit.
    pub async fn show(&self, _commit_hash: CommitHash) -> Result<CommitInfo> {
        todo!()
    }

    /// Add remote repositories according to current peer information
    pub async fn add_remote_repositories(&mut self) -> Result<()> {
        let this = self.inner.as_mut().unwrap();
        for peer in &this.config.peers {
            let port = if let Some(p) = peer.ports.get("repository") {
                p
            } else {
                continue;
            };
            let url = format!("git://{}:{port}/", peer.address.ip());
            this.repository
                .get_raw()
                .write()
                .await
                .add_remote(peer.name.clone(), url)
                .await?;
        }
        Ok(())
    }

    pub async fn serve(
        self,
        config: ServerConfig,
        git_hook_verifier: simperby_repository::server::PushVerifier,
    ) -> Result<tokio::task::JoinHandle<Result<()>>> {
        let this = self.inner.unwrap();
        let network_config = ServerNetworkConfig {
            port: config.governance_port,
        };
        let dms = this.governance.get_dms();
        let t1 = async move { Dms::serve(dms, network_config).await.unwrap() };

        let network_config = ServerNetworkConfig {
            port: config.consensus_port,
        };
        let dms = this.consensus.get_dms();
        let t2 = async move { Dms::serve(dms, network_config).await.unwrap() };
        let t3 = async move {
            let _server = simperby_repository::server::run_server(
                &this.path,
                config.repository_port,
                git_hook_verifier,
            )
            .await;
            std::future::pending::<()>().await;
        };

        Ok(tokio::spawn(async move {
            futures::future::join3(t1, t2, t3).await;
            Ok(())
        }))
    }

    pub async fn update(&mut self) -> Result<()> {
        let this = self.inner.as_mut().unwrap();
        let network_config = ClientNetworkConfig {
            peers: this.config.peers.clone(),
        };
        Dms::fetch(this.governance.get_dms(), &network_config).await?;
        Dms::fetch(this.consensus.get_dms(), &network_config).await?;
        this.repository.get_raw().write().await.fetch_all().await?;
        this.repository.sync_all().await?;

        // Update governance
        this.governance.update().await?;
        for (agenda_hash, agenda_proof) in this.governance.get_eligible_agendas().await? {
            this.repository
                .approve(&agenda_hash, agenda_proof.proof, get_timestamp())
                .await?;
        }

        // Update consensus
        this.consensus.update().await?;
        for (_, block_hash) in this.repository.read_blocks().await? {
            this.consensus
                .register_verified_block_hash(block_hash)
                .await?;
        }
        Ok(())
    }

    pub async fn broadcast(&mut self) -> Result<()> {
        let this = self.inner.as_mut().unwrap();
        let network_config = ClientNetworkConfig {
            peers: this.config.peers.clone(),
        };
        this.governance.flush().await?;
        Dms::broadcast(this.governance.get_dms(), &network_config).await?;
        this.consensus.flush().await?;
        Dms::broadcast(this.consensus.get_dms(), &network_config).await?;
        this.repository.broadcast().await?;
        Ok(())
    }
}
