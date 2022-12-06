use super::*;
use thiserror::Error;

async fn add_remotes<T: RawRepository>(
    this: &mut DistributedRepository<T>,
    known_peers: &[Peer],
) -> Result<(), Error> {
    for peer in known_peers {
        let remote_name = peer.name.clone();
        let remote_url = format!("git://{}/repo", peer.address.ip());
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
async fn retrieve_fps<T: RawRepository>(
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
async fn retrieve_local_branches<T: RawRepository>(
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
enum CommitError {
    #[error("raw repo error: {0}")]
    Raw(#[from] raw::Error),
    #[error("failed to parse commit ({1}): {0}")]
    Commit(anyhow::Error, CommitHash),
}

async fn read_commits<T: RawRepository>(
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

pub async fn fetch<T: RawRepository>(
    this: &mut DistributedRepository<T>,
    _network_config: &NetworkConfig,
    known_peers: &[Peer],
) -> Result<(), Error> {
    add_remotes(this, known_peers).await?;
    this.raw.fetch_all().await?;

    let remote_branches = this.raw.list_remote_tracking_branches().await?;
    let finalization_proofs = retrieve_fps(this, &remote_branches).await?;
    // Retrieve local branches to skip if the remote tracking branch is already in the local repo.
    let local_branches = retrieve_local_branches(&this.raw).await?;

    let mut last_finalized_commit_hash =
        this.raw.locate_branch(FINALIZED_BRANCH_NAME.into()).await?;
    let mut last_finalization_proof = format::fp_from_semantic_commit(
        this.raw
            .read_semantic_commit(this.raw.locate_branch(FP_BRANCH_NAME.into()).await?)
            .await?,
    )?;
    let last_header = this.get_last_finalized_block_header().await?;
    let reserved_state = this.get_reserved_state().await?;

    let mut next_step_branches = Vec::new();

    // Step 1: update finalization
    'branch_loop: for (remote_name, branch_name, commit_hash) in remote_branches {
        let branch_displayed = format!("{}/{}(at {})", remote_name, branch_name, commit_hash);

        // Skip if the branch is `fp`.
        if branch_name == FP_BRANCH_NAME {
            continue;
        }

        // Check if the branch is already in the local repo.
        if local_branches
            .iter()
            .any(|(_, local_branch_commit_hash)| *local_branch_commit_hash == commit_hash)
        {
            info!(
                "skip {}: already tracked by local repository",
                branch_displayed
            );
            continue;
        }

        // Check if the branch is rebased on the finalized branch.
        if last_finalized_commit_hash
            != this
                .raw
                .find_merge_base(last_finalized_commit_hash, commit_hash)
                .await?
        {
            info!("remote tracking branch outdated: {}", branch_displayed);
            continue;
        }

        // Reads the commits in the branch.
        let commits = match read_commits(this, last_finalized_commit_hash, commit_hash).await {
            Ok(x) => x,
            Err(CommitError::Commit(error, commit)) => {
                warn!("failed to parse commit {}: {}", commit, error);
                continue;
            }
            Err(e) => return Err(e.into()),
        };

        // Verify all the incoming commits
        let mut csv = CommitSequenceVerifier::new(last_header.clone(), reserved_state.clone())
            .expect("finalized branch is not accepted by CSV");
        for (new_commit, new_commit_hash) in &commits {
            if let Err(e) = csv.apply_commit(new_commit) {
                warn!(
                    "commit sequence verification failed for branch {}: {} at {}",
                    branch_displayed, e, new_commit_hash
                );
                continue 'branch_loop;
            }
        }

        next_step_branches.push((
            remote_name,
            branch_name,
            commit_hash,
            commits.last().unwrap().0.clone(),
        ));

        // Store the block commits (including the currently finalized one)
        let block_commit_hashes = std::iter::once(last_finalized_commit_hash)
            .chain(
                commits
                    .iter()
                    .filter_map(|(commit, hash)| matches!(commit, Commit::Block(_)).then(|| *hash)),
            )
            .collect::<Vec<_>>();

        // Try to finalize the last block with the known finalization proofs.
        let mut hit = false;
        for proof in &finalization_proofs {
            if csv.verify_last_header_finalization(&proof.proof).is_ok() {
                last_finalization_proof = proof.clone();
                last_finalized_commit_hash = *block_commit_hashes.last().unwrap();
                hit = true;
                break;
            }
        }

        // If failed, finalize the second to last block.
        if !hit && block_commit_hashes.len() > 2 {
            let last_block = csv.get_block_headers().last().unwrap().clone();
            last_finalization_proof = LastFinalizationProof {
                proof: last_block.prev_block_finalization_proof.clone(),
                height: last_block.height - 1,
            };
            last_finalized_commit_hash = block_commit_hashes[block_commit_hashes.len() - 2];
        }
    }

    // Step 2: create `a` and `b` branches.
    for (remote_name, branch_name, commit_hash, commit) in next_step_branches {
        let branch_displayed = format!("{}/{}(at {})", remote_name, branch_name, commit_hash);
        match &commit {
            Commit::Agenda(agenda) => {
                if agenda.height == last_finalization_proof.height + 1 {
                    let branch_name = format!(
                        "a-{:?}",
                        commit
                            .to_hash256()
                            .to_string()
                            .truncate(COMMIT_TITLE_HASH_DIGITS)
                    );
                    this.raw.create_branch(branch_name, commit_hash).await?;
                }
            }
            Commit::AgendaProof(agenda_proof) => {
                if agenda_proof.height == last_finalization_proof.height + 1 {
                    let branch_name = format!(
                        "a-{:?}",
                        commit
                            .to_hash256()
                            .to_string()
                            .truncate(COMMIT_TITLE_HASH_DIGITS)
                    );
                    this.raw.create_branch(branch_name, commit_hash).await?;
                }
            }
            Commit::Block(block_header) => {
                if block_header.height == last_finalization_proof.height + 1 {
                    let branch_name = format!(
                        "b-{:?}",
                        commit
                            .to_hash256()
                            .to_string()
                            .truncate(COMMIT_TITLE_HASH_DIGITS)
                    );
                    this.raw.create_branch(branch_name, commit_hash).await?;
                }
            }
            x => warn!("incomplete remote branch {}: got {:?}", branch_displayed, x),
        }
    }

    // Step 3: update finalization
    this.raw.checkout_clean().await?;
    this.raw
        .move_branch(FINALIZED_BRANCH_NAME.into(), last_finalized_commit_hash)
        .await?;
    this.raw
        .move_branch(FP_BRANCH_NAME.into(), last_finalized_commit_hash)
        .await?;
    this.raw.checkout(FP_BRANCH_NAME.into()).await?;
    this.raw
        .create_semantic_commit(format::fp_to_semantic_commit(&last_finalization_proof))
        .await?;
    Ok(())
}
