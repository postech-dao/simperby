mod storage;
pub mod types;

use eyre::eyre;
use eyre::Result;
use simperby_consensus::*;
use simperby_core::utils::get_timestamp;
use simperby_core::*;
use simperby_governance::*;
use simperby_network::dms::PeerStatus;
use simperby_network::peers::Peers;
use simperby_network::*;
use simperby_repository::raw::RawRepository;
use simperby_repository::*;
use std::net::SocketAddrV4;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

pub use simperby_consensus;
pub use simperby_core;
pub use simperby_governance;
pub use simperby_network;
pub use simperby_repository;

pub use crate::types::*;

/// A client for a single height.
struct ClientInner {
    config: types::Config,
    auth: Auth,
    path: String,
    repository: DistributedRepository,
    governance: Governance,
    consensus: Consensus,
    peers: Peers,
}

/// An instance of Simperby client (a.k.a. a 'node').
pub struct Client {
    inner: Option<ClientInner>,
}

impl Client {
    pub async fn dump_genesis(path: &str) -> Result<()> {
        let (rs, keys) = test_utils::generate_standard_genesis(4);

        simperby_repository::raw::reserved_state::write_reserved_state(path, &rs)
            .await
            .unwrap();
        let keys = serde_spb::to_string(&keys)?;
        fs::write(format!("{}/{}", path, "keys.json"), keys).await?;
        Ok(())
    }
    pub async fn genesis(path: &str) -> Result<()> {
        let repository = RawRepository::open(path).await?;
        DistributedRepository::genesis(repository).await?;
        Ok(())
    }

    pub async fn init(path: &str) -> Result<()> {
        storage::init(path).await?;
        Ok(())
    }

    pub async fn open(path: &str, config: types::Config, auth: Auth) -> Result<Self> {
        let (governance_dms, consensus_dms, consensus_state, repository_dms, peers) =
            storage::open(path, config.clone(), auth.clone()).await?;
        let repository = DistributedRepository::new(
            Some(Arc::new(RwLock::new(repository_dms))),
            Arc::new(RwLock::new(RawRepository::open(path).await?)),
            simperby_repository::Config {
                long_range_attack_distance: 3,
            },
            Some(auth.private_key.clone()),
        )
        .await?;

        let lfi = repository.read_last_finalization_info().await?;
        let agendas = repository.read_agendas().await?;
        Ok(Self {
            inner: Some(ClientInner {
                config,
                auth: auth.clone(),
                path: path.to_string(),
                repository,
                governance: Governance::new(
                    Arc::new(RwLock::new(governance_dms)),
                    lfi.clone(),
                    agendas.into_iter().map(|(_, hash)| hash).collect(),
                )
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
                peers,
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
                let peers = this.peers.list_peers().await?;
                drop(this);
                storage::clear(&path).await?;
                storage::init(&path).await?;
                let mut this = Self::open(&path, config, auth).await?.inner.unwrap();
                for peer in peers {
                    this.peers.add_peer(peer.name, peer.address).await?;
                }
                self.inner = Some(this);
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
    ///
    /// For every type of commit,
    /// 1. Show the content.
    /// 2. Show the hash of it.
    ///
    /// For an agenda, show the governance status.
    /// For a block, show the consensus status projected on this block.
    /// For an extra-agenda transaction and a chat log, TODO.
    pub async fn show(&self, _commit_hash: CommitHash) -> Result<CommitInfo> {
        todo!()
    }

    pub async fn serve(
        self,
        config: ServerConfig,
        git_hook_verifier: simperby_repository::server::PushVerifier,
    ) -> Result<tokio::task::JoinHandle<Result<()>>> {
        let this = self.inner.unwrap();

        // Serve peers
        let peers = Arc::new(RwLock::new(this.peers));
        let port_map = vec![
            (
                simperby_network::keys::port_key_dms::<simperby_governance::Vote>(),
                config.governance_port,
            ),
            (
                simperby_network::keys::port_key_dms::<simperby_consensus::ConsensusMessage>(),
                config.consensus_port,
            ),
            ("repository".to_owned(), config.repository_port),
        ]
        .into_iter()
        .collect();
        let network_config = ServerNetworkConfig {
            port: config.peers_port,
        };
        let t0 = async move { Peers::serve(peers, port_map, network_config).await.unwrap() };

        // Serve governance
        let network_config = ServerNetworkConfig {
            port: config.governance_port,
        };
        let dms = this.governance.get_dms();
        let t1 = async move { Dms::serve(dms, network_config).await.unwrap() };

        // Serve consensus
        let network_config = ServerNetworkConfig {
            port: config.consensus_port,
        };
        let dms = this.consensus.get_dms();
        let t2 = async move { Dms::serve(dms, network_config).await.unwrap() };

        // Serve repository
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
            futures::future::join4(t0, t1, t2, t3).await;
            Ok(())
        }))
    }

    pub async fn update(&mut self) -> Result<()> {
        let this = self.inner.as_mut().unwrap();
        let network_config = ClientNetworkConfig {
            peers: this.peers.list_peers().await?,
        };
        Dms::fetch(this.governance.get_dms(), &network_config).await?;
        Dms::fetch(this.consensus.get_dms(), &network_config).await?;
        this.repository
            .get_raw()
            .write()
            .await
            .fetch_all(true)
            .await?;
        this.repository.sync_all().await?;

        let agendas = this.repository.read_agendas().await?;
        for (_, agenda_hash) in agendas {
            this.governance
                .register_verified_agenda_hash(agenda_hash)
                .await?;
        }

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
            peers: this.peers.list_peers().await?,
        };
        this.governance.flush().await?;
        Dms::broadcast(this.governance.get_dms(), &network_config).await?;
        this.consensus.flush().await?;
        Dms::broadcast(this.consensus.get_dms(), &network_config).await?;
        this.repository.broadcast().await?;
        Ok(())
    }

    pub async fn add_peer(&mut self, name: MemberName, address: SocketAddrV4) -> Result<()> {
        let this = self.inner.as_mut().unwrap();
        this.peers.add_peer(name, address).await?;
        Ok(())
    }

    pub async fn remove_peer(&mut self, name: MemberName) -> Result<()> {
        let this = self.inner.as_mut().unwrap();
        this.peers.remove_peer(name).await?;
        Ok(())
    }

    pub async fn get_peer_list(&self) -> Result<Vec<Peer>> {
        let this = self.inner.as_ref().unwrap();
        this.peers.list_peers().await
    }

    pub async fn update_peer(&mut self) -> Result<()> {
        let this = self.inner.as_mut().unwrap();
        this.peers.update().await?;
        self.add_remote_repositories().await?;
        Ok(())
    }

    /// Adds remote repositories according to current peer information.
    async fn add_remote_repositories(&mut self) -> Result<()> {
        let this = self.inner.as_mut().unwrap();
        for peer in this.peers.list_peers().await? {
            let port = if let Some(p) = peer.ports.get("repository") {
                p
            } else {
                continue;
            };
            let url = format!("git://{}:{port}/", peer.address.ip());
            // TODO: skip only "already exists" error
            let _ = this
                .repository
                .get_raw()
                .write()
                .await
                .add_remote(peer.name.clone(), url)
                .await;
        }
        Ok(())
    }

    pub async fn get_peer_status(&self) -> Result<Vec<PeerStatus>> {
        let this = self.inner.as_ref().unwrap();
        let network_config = ClientNetworkConfig {
            peers: this.peers.list_peers().await?,
        };
        let result = Dms::get_peer_status(this.governance.get_dms(), &network_config).await?;
        Ok(result)
    }
}
