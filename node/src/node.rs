use super::*;
use eyre::eyre;
use simperby_consensus::{Consensus, ConsensusParameters, ProgressResult};
use simperby_network::primitives::{GossipNetwork, Storage};
use simperby_network::NetworkConfig;
use simperby_network::{dms, storage::StorageImpl, Dms, Peer, SharedKnownPeers};
use simperby_repository::raw::{RawRepository, RawRepositoryImpl};
use simperby_repository::DistributedRepository;
use std::collections::HashMap;

fn get_timestamp() -> Timestamp {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as Timestamp
}

pub struct Node<N: GossipNetwork, S: Storage, R: RawRepository> {
    config: Config,
    repository: DistributedRepository<R>,
    governance: Governance<N, S>,
    consensus: Consensus<N, S>,

    last_reserved_state: ReservedState,
    #[allow(dead_code)]
    last_finalized_header: BlockHeader,

    path: String,
    network_config: NetworkConfig,
}

impl SimperbyNode {
    pub async fn initialize(config: Config, path: &str) -> Result<Self> {
        // Step 0: initialize the repository module
        let peers: Vec<Peer> =
            serde_spb::from_str(&tokio::fs::read_to_string(&format!("{path}/peers.json")).await?)?;
        let peers = SharedKnownPeers::new_static(peers.clone());
        let raw_repository = RawRepositoryImpl::open(&format!("{path}/repository/repo")).await?;
        let repository = DistributedRepository::new(
            raw_repository,
            simperby_repository::Config {
                mirrors: config.public_repo_url.clone(),
                long_range_attack_distance: 3,
            },
            peers.clone(),
        )
        .await?;

        // Step 1: initialize configs
        let last_finalized_header = repository.get_last_finalized_block_header().await?;
        let reserved_state = repository.get_reserved_state().await?;
        let governance_dms_key = simperby_governance::generate_dms_key(&last_finalized_header);
        let consensus_dms_key = simperby_consensus::generate_dms_key(&last_finalized_header);
        let network_config = NetworkConfig {
            network_id: reserved_state.genesis_info.chain_name.clone(),
            ports: vec![
                (
                    format!("dms-{}", governance_dms_key.clone()),
                    config.governance_port,
                ),
                (
                    format!("dms-{}", consensus_dms_key.clone()),
                    config.consensus_port,
                ),
                ("repository".to_owned(), config.repository_port),
            ]
            .into_iter()
            .collect(),
            members: reserved_state
                .members
                .iter()
                .map(|m| m.public_key.clone())
                .collect(),
            public_key: config.public_key.clone(),
            private_key: config.private_key.clone(),
        };
        let dms_config = dms::Config {
            fetch_interval: Some(std::time::Duration::from_millis(500)),
            broadcast_interval: Some(std::time::Duration::from_millis(500)),
            network_config: network_config.clone(),
        };

        // Step 2: initialize the governance module
        let dms_path = format!("{path}/governance/dms");
        StorageImpl::create(&dms_path).await.unwrap();
        let storage = StorageImpl::open(&dms_path).await.unwrap();
        let dms = Dms::new(
            storage,
            governance_dms_key,
            dms_config.clone(),
            peers.clone(),
        )
        .await?;
        let governance = Governance::new(dms, Some(config.private_key.clone())).await?;

        // Step 3: initialize the consensus module
        let dms_path = format!("{path}/consensus/dms");
        StorageImpl::create(&dms_path).await.unwrap();
        let storage = StorageImpl::open(&dms_path).await.unwrap();
        let dms = Dms::new(
            storage,
            consensus_dms_key,
            dms_config.clone(),
            peers.clone(),
        )
        .await?;
        let state_path = format!("{path}/consensus/state");
        StorageImpl::create(&state_path).await.unwrap();
        let consensus_state_storage = StorageImpl::open(&state_path).await.unwrap();
        let consensus = Consensus::new(
            dms,
            consensus_state_storage,
            last_finalized_header.clone(),
            // TODO: replace params and timestamp with proper values
            ConsensusParameters {
                timeout_ms: 10000000,
                repeat_round_for_first_leader: 100,
            },
            0,
            Some(config.private_key.clone()),
        )
        .await?;
        Ok(Self {
            config,
            repository,
            governance,
            consensus,
            last_reserved_state: reserved_state,
            last_finalized_header,
            path: path.to_owned(),
            network_config,
        })
    }

    pub fn get_raw_repo(&self) -> &impl RawRepository {
        self.repository.get_raw()
    }

    pub fn get_raw_repo_mut(&mut self) -> &mut impl RawRepository {
        self.repository.get_raw_mut()
    }

    /// TODO: revise this interface
    pub fn network_config(&self) -> &NetworkConfig {
        &self.network_config
    }

    /// Synchronizes the `finalized` branch to the given commit.
    pub async fn sync(&mut self, _commmit: CommitHash) -> Result<()> {
        todo!()
    }

    /// Cleans the repository, removing all the outdated commits.
    pub async fn clean(&mut self, _hard: bool) -> Result<()> {
        self.repository.clean().await
    }

    /// Creates a block commit on the `work` branch.
    pub async fn create_block(&mut self) -> Result<CommitHash> {
        let (header, commit_hash) = self
            .repository
            .create_block(self.config.public_key.clone())
            .await?;
        // automatically set as my proposal
        self.consensus
            .register_verified_block_hash(header.to_hash256())
            .await?;
        self.consensus
            .set_proposal_candidate(header.to_hash256(), get_timestamp())
            .await?;
        Ok(commit_hash)
    }

    /// Creates an agenda commit on the `work` branch.
    pub async fn create_agenda(&mut self) -> Result<CommitHash> {
        let (_, commit_hash) = self
            .repository
            .create_agenda(self.config.public_key.clone())
            .await?;
        Ok(commit_hash)
    }

    /// Creates an extra-agenda transaction on the `work` branch.
    pub async fn create_extra_agenda_transaction(
        &mut self,
        _tx: ExtraAgendaTransaction,
    ) -> Result<()> {
        unimplemented!()
    }

    /// Votes on the agenda corresponding to the given `agenda_commit` and propagates the result.
    pub async fn vote(&mut self, agenda_commit: CommitHash) -> Result<()> {
        let valid_agendas = self.repository.get_agendas().await?;
        let agenda_hash = if let Some(x) = valid_agendas.iter().find(|(x, _)| *x == agenda_commit) {
            x.1
        } else {
            return Err(eyre!(
                "the given commit hash {} is not one of the valid agendas",
                agenda_commit
            ));
        };
        self.repository.vote(agenda_commit).await?;
        self.governance.vote(agenda_hash).await?;
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
    pub async fn show(&self, commit_hash: CommitHash) -> Result<CommitInfo> {
        let semantic_commit = self
            .repository
            .get_raw()
            .read_semantic_commit(commit_hash)
            .await?;
        let commit = simperby_repository::format::from_semantic_commit(semantic_commit.clone())?;
        let result = match commit {
            Commit::Block(block_header) => CommitInfo::Block {
                semantic_commit,
                block_header,
            },
            Commit::Agenda(agenda) => CommitInfo::Agenda {
                semantic_commit,
                agenda: agenda.clone(),
                voters: self
                    .governance
                    .read()
                    .await?
                    .votes
                    .get(&agenda.to_hash256())
                    .unwrap_or(&Default::default())
                    .iter()
                    .filter_map(|(public_key, _)| {
                        self.last_reserved_state
                            .query_name(public_key)
                            .map(|x| (x, 0))
                    })
                    .collect(), // TODO
            },
            Commit::AgendaProof(agenda_proof) => CommitInfo::AgendaProof {
                semantic_commit,
                agenda_proof,
            },
            x => CommitInfo::Unknown {
                semantic_commit,
                msg: format!("{x:?}"),
            },
        };
        Ok(result)
    }

    /// Makes a progress for the consensus, returning the result.
    ///
    /// TODO: it has to consume the object if finalized.
    pub async fn progress_for_consensus(&mut self) -> Result<String> {
        let result = self.consensus.progress(get_timestamp()).await?;
        for result in result.iter() {
            if let ProgressResult::Finalized(hash, _, proof) = result {
                self.repository.sync(hash, proof).await?;
            }
        }
        Ok(format!("{result:?}"))
    }

    /// Gets the current status of the consensus.
    pub async fn get_consensus_status(&self) -> Result<ConsensusStatus> {
        todo!()
    }

    /// Gets the current status of the p2p network.
    pub async fn get_network_status(&self) -> Result<NetworkStatus> {
        unimplemented!()
    }

    pub async fn serve(self, ms: u64) -> Result<Self> {
        let repository_port = self.config.repository_port;

        let t1 = tokio::spawn(async move { self.governance.serve(ms).await.unwrap() });
        let t2 = tokio::spawn(async move { self.consensus.serve(ms).await.unwrap() });
        let path = self.path.clone();
        let t3 = tokio::spawn(async move {
            let server = simperby_repository::server::run_server_legacy(
                &format!("{path}/repository"),
                repository_port,
            )
            .await;
            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
            drop(server);
        });

        let governance = t1.await?;
        let consensus = t2.await?;
        t3.await?;

        Ok(Self {
            governance,
            consensus,
            config: self.config,
            repository: self.repository,
            last_reserved_state: self.last_reserved_state,
            last_finalized_header: self.last_finalized_header,
            path: self.path,
            network_config: self.network_config,
        })
    }

    pub async fn fetch(&mut self) -> Result<()> {
        let t1 = async { self.governance.fetch().await };
        let t2 = async { self.consensus.fetch().await };
        let t3 = async { self.repository.fetch().await };
        futures::try_join!(t1, t2, t3)?;

        // Update governance
        let governance_set = self
            .last_reserved_state
            .get_governance_set()
            .unwrap()
            .into_iter()
            .collect::<HashMap<_, _>>();
        let governance_state = self.governance.read().await?;
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
        let total_voting_power = governance_set.values().sum::<VotingPower>();
        for (agenda, voted_power) in votes {
            if voted_power * 2 > total_voting_power {
                // TODO: handle this error
                let _ = self
                    .repository
                    .approve(
                        &agenda,
                        governance_state.votes[&agenda]
                            .iter()
                            .map(|(k, s)| TypedSignature::new(s.clone(), k.clone()))
                            .collect(),
                    )
                    .await;
            }
        }

        // Update consensus
        for (_, block_hash) in self.repository.get_blocks().await? {
            self.consensus
                .register_verified_block_hash(block_hash)
                .await?;
        }
        Ok(())
    }

    /// Broadcasts all the local messages and reports the result.
    pub async fn broadcast(&mut self) -> Result<Vec<String>> {
        let t1 = async { self.governance.broadcast().await };
        let t2 = async { self.consensus.broadcast().await };
        let t3 = async { self.repository.broadcast().await };
        futures::try_join!(t1, t2, t3)?;
        // TODO: report the result
        Ok(vec![])
    }
}
