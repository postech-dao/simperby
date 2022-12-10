use super::*;
use thiserror::Error;

pub async fn add_remotes<T: RawRepository>(
    this: &mut DistributedRepository<T>,
    known_peers: &[Peer],
) -> Result<(), Error> {
    for peer in known_peers {
        let remote_name = peer.name.clone();
        let remote_url = format!(
            "git://{}:{}/repo",
            peer.address.ip(),
            // 9418 is the default port for git server
            peer.ports.get("repository").unwrap_or(&9418)
        );
        if let Err(err) = this.raw.add_remote(remote_name, remote_url.clone()).await {
            warn!("failed to add remote({}): {}", remote_url, err);
        }
    }
    for (i, mirror) in this.config.mirrors.iter().enumerate() {
        let remote_name = format!("mirror_{}", i);
        if let Err(err) = this.raw.add_remote(remote_name, mirror.clone()).await {
            warn!("failed to add remote({}): {}", mirror, err);
        }
    }
    Ok(())
}

/// Retrieve all the finalization proofs
pub async fn retrieve_fps<T: RawRepository>(
    this: &DistributedRepository<T>,
    remote_branches: &[(String, String, CommitHash)],
) -> Result<Vec<LastFinalizationProof>, Error> {
    let mut result = Vec::new();
    for (remote_name, branch_name, commit_hash) in remote_branches {
        let branch_displayed = format!("{}/{}(at {})", remote_name, branch_name, commit_hash);

        // Skip if the branch is not `fp`.
        if branch_name != FP_BRANCH_NAME {
            continue;
        }

        let fp = this.raw.read_semantic_commit(*commit_hash).await?;
        let fp = match format::fp_from_semantic_commit(fp) {
            Ok(x) => x,
            Err(err) => {
                warn!("fp branch is invalid {}: {}", branch_displayed, err);
                continue;
            }
        };
        result.push(fp);
    }
    Ok(result)
}

/// Retrieve all local branches
pub async fn retrieve_local_branches<T: RawRepository>(
    raw: &T,
) -> Result<HashSet<(Branch, CommitHash)>, Error> {
    let local_branches = raw.list_branches().await?;
    let mut result = HashSet::new();
    // TODO: making this concurrent causes a god damn lifetime annoying error
    for b in local_branches {
        result.insert((b.clone(), raw.locate_branch(b).await?));
    }
    Ok(result)
}

#[derive(Debug, Error)]
pub enum CommitError {
    #[error("raw repo error: {0}")]
    Raw(#[from] raw::Error),
    #[error("failed to parse commit ({1}): {0}")]
    Commit(eyre::Error, CommitHash),
}

/// Reads the git commits to `Commit`s, from the very next commit of ancestor to descendant.
/// `ancestor` not included, `descendant` included.
/// It fails if the two commits are the same.
/// It fails if the ancestor is not the merge base of the two commits.
pub async fn read_commits<T: RawRepository>(
    this: &DistributedRepository<T>,
    ancestor: CommitHash,
    descendant: CommitHash,
) -> Result<Vec<(Commit, CommitHash)>, CommitError> {
    let commits = this.raw.query_commit_path(ancestor, descendant).await?;
    let commits = stream::iter(commits.iter().cloned().map(|c| {
        let raw = &this.raw;
        async move { raw.read_semantic_commit(c).await.map(|x| (x, c)) }
    }))
    .buffered(256)
    .collect::<Vec<_>>()
    .await;
    let commits = commits.into_iter().collect::<Result<Vec<_>, _>>()?;
    let commits = commits
        .into_iter()
        .map(|(commit, hash)| {
            from_semantic_commit(commit)
                .map_err(|e| (e, hash))
                .map(|x| (x, hash))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|(e, c)| CommitError::Commit(e, c))?;
    Ok(commits)
}

/// Reads a single commit.
pub async fn read_commit<T: RawRepository>(
    this: &DistributedRepository<T>,
    commit: CommitHash,
) -> Result<Commit, Error> {
    let commit = this.raw.read_semantic_commit(commit).await?;
    from_semantic_commit(commit)
}
