use super::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommitError {
    #[error("raw repo error: {0}")]
    Raw(#[from] raw::Error),
    #[error("failed to parse commit ({1}): {0}")]
    Commit(eyre::Error, CommitHash),
    #[error("reserved state error: {0}")]
    ReservedState(#[from] super::Error),
}

pub async fn read_local_branches(
    raw: &RawRepository,
) -> Result<HashSet<(Branch, CommitHash)>, Error> {
    let local_branches = raw.list_branches().await?;
    let mut result = HashSet::new();
    // TODO: making this concurrent causes a god damn lifetime annoying error
    for b in local_branches {
        result.insert((b.clone(), raw.locate_branch(b).await?));
    }
    Ok(result)
}

pub async fn get_last_finalized_block_commit_hash(
    raw: &RawRepository,
) -> Result<CommitHash, Error> {
    raw.locate_branch(FINALIZED_BRANCH_NAME.into())
        .await
        .map_err(|e| match e {
            raw::Error::NotFound(_) => {
                eyre!(IntegrityError::new(
                    "cannot locate `finalized` branch".to_string()
                ))
            }
            _ => eyre!(e),
        })
}

pub async fn read_last_finalized_reserved_state(
    raw: &RawRepository,
) -> Result<ReservedState, Error> {
    Ok(raw
        .read_reserved_state_at_commit(get_last_finalized_block_commit_hash(raw).await?)
        .await?)
}

pub async fn read_last_finalized_block_header(raw: &RawRepository) -> Result<BlockHeader, Error> {
    let commit_hash = raw.locate_branch(FINALIZED_BRANCH_NAME.into()).await?;
    let semantic_commit = raw.read_semantic_commit(commit_hash).await?;
    let commit = format::from_semantic_commit(semantic_commit).map_err(|e| eyre!(e))?;
    if let Commit::Block(block_header) = commit {
        Ok(block_header)
    } else {
        Err(eyre!(IntegrityError {
            msg: "`finalized` branch is not on a block".to_owned(),
        }))
    }
}

pub async fn read_last_finalization_proof(
    raw: &RawRepository,
) -> Result<LastFinalizationProof, Error> {
    if let Ok(last_finalization_proof) = format::fp_from_semantic_commit(
        raw.read_semantic_commit(raw.locate_branch(FP_BRANCH_NAME.into()).await.map_err(
            |e| match e {
                raw::Error::NotFound(_) => {
                    eyre!(IntegrityError::new("cannot locate `fp` branch".to_string()))
                }
                _ => eyre!(e),
            },
        )?)
        .await?,
    ) {
        Ok(last_finalization_proof)
    } else {
        Err(eyre!(IntegrityError {
            msg: "`fp` branch is not on a finalization proof".to_owned(),
        }))
    }
}

pub async fn read_last_finalization_info(raw: &RawRepository) -> Result<FinalizationInfo, Error> {
    let header = read_last_finalized_block_header(raw).await?;
    let commit_hash = get_last_finalized_block_commit_hash(raw).await?;
    let reserved_state = read_last_finalized_reserved_state(raw).await?;
    let proof = read_last_finalization_proof(raw).await?.proof;
    Ok(FinalizationInfo {
        header,
        commit_hash,
        reserved_state,
        proof,
    })
}

pub async fn read_commits(
    raw: &RawRepository,
    ancestor: CommitHash,
    descendant: CommitHash,
) -> Result<Vec<(Commit, CommitHash)>, CommitError> {
    let commits = raw.query_commit_path(ancestor, descendant).await?;
    let commits = stream::iter(
        commits
            .iter()
            .cloned()
            .map(|c| async move { raw.read_semantic_commit(c).await.map(|x| (x, c)) }),
    )
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

/// Reads the sequence of commits from the last finalized block to the given commit,
/// and verifies them, and returns the CSV that all commits have been applied on.
/// It does not accept a last finalization proof commit.
///
/// - Returns `Ok(Ok(commit))` if the branch (taking the given commit as a tip) is valid.
/// - Returns `Ok(Err(error))` if the branch is invalid.
/// - Returns `Err(error)` if there was an error reading the commits.
pub async fn read_and_verify_commits_from_last_finalized_block(
    raw: &RawRepository,
    commit_hash: CommitHash,
) -> Result<Result<CommitSequenceVerifier, Error>, Error> {
    let lfi = read_last_finalization_info(raw).await?;

    if raw.find_merge_base(lfi.commit_hash, commit_hash).await? != lfi.commit_hash {
        return Ok(Err(eyre!(
            "the given commit is not a descendant of the last finalized block."
        )));
    }

    if format::fp_from_semantic_commit(raw.read_semantic_commit(commit_hash).await?).is_ok() {
        return Ok(Err(eyre!(
            "the given commit is a finalization proof commit."
        )));
    }

    let commits = read_commits(raw, lfi.commit_hash, commit_hash).await?;
    let mut csv = CommitSequenceVerifier::new(lfi.header, lfi.reserved_state).map_err(|e| {
        IntegrityError::new(format!("finalized branch is not accepted by CSV: {e}"))
    })?;

    for commit in commits {
        if let Err(e) = csv.apply_commit(&commit.0) {
            return Ok(Err(eyre!(e)));
        }
    }
    Ok(Ok(csv))
}

pub async fn read_commit(raw: &RawRepository, commit_hash: CommitHash) -> Result<Commit, Error> {
    let semantic_commit = raw.read_semantic_commit(commit_hash).await?;
    format::from_semantic_commit(semantic_commit).map_err(|e| eyre!(e))
}

pub async fn read_agendas(raw: &RawRepository) -> Result<Vec<(CommitHash, Hash256)>, Error> {
    let mut agendas: Vec<(CommitHash, Hash256)> = vec![];
    let branches = read_local_branches(raw).await?;
    let last_header_commit_hash = raw.locate_branch(FINALIZED_BRANCH_NAME.into()).await?;
    for (branch, branch_commit_hash) in branches {
        // Check if the branch is an agenda branch
        if branch.as_str().starts_with("a-") {
            // Check if the agenda branch is rebased on top of the `finalized` branch
            let find_merge_base_result = raw
                .find_merge_base(last_header_commit_hash, branch_commit_hash)
                .await
                .map_err(|e| match e {
                    raw::Error::NotFound(_) => {
                        eyre!(IntegrityError::new(format!(
                            "cannot find merge base for branch {branch} and finalized branch"
                        )))
                    }
                    _ => eyre!(e),
                })?;

            if last_header_commit_hash != find_merge_base_result {
                log::warn!(
                    "branch {} should be rebased on top of the {} branch",
                    branch,
                    FINALIZED_BRANCH_NAME
                );
                continue;
            }

            // Push currently valid and height-acceptable agendas to the list
            let commits = read_commits(raw, last_header_commit_hash, branch_commit_hash).await?;
            let last_header = read_last_finalized_block_header(raw).await?;
            for (commit, hash) in commits {
                if let Commit::Agenda(agenda) = commit {
                    if agenda.height == last_header.height + 1 {
                        agendas.push((hash, agenda.to_hash256()));
                    }
                }
            }
        }
    }
    Ok(agendas)
}

pub async fn read_governance_approved_agendas(
    _raw: &RawRepository,
) -> Result<Vec<(CommitHash, Hash256)>, Error> {
    todo!()
}

pub async fn read_blocks(raw: &RawRepository) -> Result<Vec<(CommitHash, Hash256)>, Error> {
    let mut blocks: Vec<(CommitHash, Hash256)> = vec![];
    let branches = read_local_branches(raw).await?;
    let last_header_commit_hash = raw.locate_branch(FINALIZED_BRANCH_NAME.into()).await?;
    for (branch, branch_commit_hash) in branches {
        // Check if the branch is a block branch
        if branch.as_str().starts_with("b-") {
            // Check if the block branch is rebased on top of the `finalized` branch
            let find_merge_base_result = raw
                .find_merge_base(last_header_commit_hash, branch_commit_hash)
                .await
                .map_err(|e| match e {
                    raw::Error::NotFound(_) => {
                        eyre!(IntegrityError::new(format!(
                            "cannot find merge base for branch {branch} and finalized branch"
                        )))
                    }
                    _ => eyre!(e),
                })?;
            if last_header_commit_hash != find_merge_base_result {
                log::warn!(
                    "branch {} should be rebased on top of the {} branch",
                    branch,
                    FINALIZED_BRANCH_NAME
                );
                continue;
            }

            // Push currently valid and height-acceptable blocks to the list
            let commits = read_commits(raw, last_header_commit_hash, branch_commit_hash).await?;
            let last_header = read_last_finalized_block_header(raw).await?;
            for (commit, hash) in commits {
                if let Commit::Block(block_header) = commit {
                    if block_header.height == last_header.height + 1 {
                        blocks.push((hash, block_header.to_hash256()));
                    }
                }
            }
        }
    }
    Ok(blocks)
}
