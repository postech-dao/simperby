use crate::peers::Peers;
use simperby_network::keys;
use tokio::io::AsyncWriteExt;

use super::*;

fn governance_dms_path(path: &str) -> String {
    format!("{path}/.simperby/governance/dms")
}

fn consensus_dms_path(path: &str) -> String {
    format!("{path}/.simperby/consensus/dms")
}

fn consensus_state_path(path: &str) -> String {
    format!("{path}/.simperby/consensus/state")
}

fn repository_dms_path(path: &str) -> String {
    format!("{path}/.simperby/repository/dms")
}

fn peers_path(path: &str) -> String {
    format!("{path}/.simperby/peers.json")
}

pub(crate) async fn init(path: &str) -> Result<()> {
    let repository = DistributedRepository::new(
        None,
        Arc::new(RwLock::new(RawRepository::open(path).await?)),
        simperby_repository::Config {
            long_range_attack_distance: 3,
        },
        None,
    )
    .await?;
    repository.check(0).await?;

    StorageImpl::create(&governance_dms_path(path)).await?;
    StorageImpl::create(&consensus_dms_path(path)).await?;
    StorageImpl::create(&consensus_state_path(path)).await?;
    StorageImpl::create(&repository_dms_path(path)).await?;
    let mut file = tokio::fs::File::create(&peers_path(path)).await?;
    file.write_all(serde_spb::to_string(&Vec::<Peer>::new())?.as_bytes())
        .await?;
    Ok(())
}

/// `(Governance DMS, Consensus DMS, ConsensusState, Repository DMS, Peers)`.
pub(crate) async fn open(
    path: &str,
    _config: types::Config,
    auth: Auth,
) -> Result<(
    Dms<simperby_governance::Vote>,
    Dms<simperby_consensus::ConsensusMessage>,
    StorageImpl,
    Dms<simperby_repository::RepositoryMessage>,
    Peers,
)> {
    let repository = DistributedRepository::new(
        None,
        Arc::new(RwLock::new(RawRepository::open(path).await?)),
        simperby_repository::Config {
            long_range_attack_distance: 3,
        },
        Some(auth.private_key.clone()),
    )
    .await?;
    repository.check(0).await?;
    let lfi = repository.read_last_finalization_info().await?;
    let dms_members: Vec<_> = lfi
        .reserved_state
        .get_governance_set()
        .map_err(simperby_repository::IntegrityError::new)?
        .into_iter()
        .map(|x| x.0)
        .collect();

    let storage = StorageImpl::open(&governance_dms_path(path)).await?;
    let governance_dms = Dms::<simperby_governance::Vote>::new(
        storage,
        dms::Config {
            dms_key: keys::dms_key::<simperby_governance::Vote>(&lfi.header),
            members: dms_members.clone(),
        },
        auth.private_key.clone(),
    )
    .await?;
    let storage = StorageImpl::open(&consensus_dms_path(path)).await?;
    let consensus_dms = Dms::<simperby_consensus::ConsensusMessage>::new(
        storage,
        dms::Config {
            dms_key: keys::dms_key::<simperby_consensus::ConsensusMessage>(&lfi.header),
            members: dms_members.clone(),
        },
        auth.private_key.clone(),
    )
    .await?;
    let consensus_state = StorageImpl::open(&consensus_state_path(path)).await?;
    let storage = StorageImpl::open(&repository_dms_path(path)).await?;
    let repository_dms = Dms::<simperby_repository::RepositoryMessage>::new(
        storage,
        dms::Config {
            dms_key: keys::dms_key::<simperby_repository::RepositoryMessage>(&lfi.header),
            members: dms_members.clone(),
        },
        auth.private_key.clone(),
    )
    .await?;
    let lfi = repository.read_last_finalization_info().await?;
    Ok((
        governance_dms,
        consensus_dms,
        consensus_state,
        repository_dms,
        Peers::new(&peers_path(path), lfi, auth.private_key.clone()).await?,
    ))
}

pub(crate) async fn clear(path: &str) -> Result<()> {
    let _ = tokio::fs::remove_dir_all(&governance_dms_path(path)).await;
    let _ = tokio::fs::remove_dir_all(&consensus_dms_path(path)).await;
    let _ = tokio::fs::remove_dir_all(&consensus_state_path(path)).await;
    let _ = tokio::fs::remove_file(&peers_path(path)).await;
    Ok(())
}
