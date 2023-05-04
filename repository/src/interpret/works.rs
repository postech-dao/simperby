use super::*;
use read::*;

async fn advance_finalized_branch(
    raw: &mut RawRepository,
    to_be_finalized_block_commit_hash: CommitHash,
    finalization_proof: LastFinalizationProof,
) -> Result<(), Error> {
    raw.checkout_clean().await?;
    raw.move_branch(
        FINALIZED_BRANCH_NAME.into(),
        to_be_finalized_block_commit_hash,
    )
    .await?;
    raw.move_branch(FP_BRANCH_NAME.into(), to_be_finalized_block_commit_hash)
        .await?;
    raw.checkout(FP_BRANCH_NAME.into()).await?;
    raw.create_semantic_commit(format::fp_to_semantic_commit(&finalization_proof))
        .await?;
    Ok(())
}

pub async fn sync(
    raw: &mut RawRepository,
    tip_commit_hash: CommitHash,
) -> Result<Result<(), String>, Error> {
    let lfi = read_last_finalization_info(raw).await?;
    let mut csv = CommitSequenceVerifier::new(lfi.header.clone(), lfi.reserved_state.clone())
        .map_err(|e| {
            IntegrityError::new(format!("finalized branch is not accepted by CSV: {e}"))
        })?;

    if raw
        .find_merge_base(lfi.commit_hash, tip_commit_hash)
        .await?
        != lfi.commit_hash
    {
        return Ok(Err(
            "the received branch tip commit is not a descendant of the last finalized block."
                .to_owned(),
        ));
    }

    // If the branch ends with a finalization proof commit
    if let Ok(last_finalization_proof) =
        format::fp_from_semantic_commit(raw.read_semantic_commit(tip_commit_hash).await?)
    {
        // Now we consider the direct parent as the received commit
        // (fp is not treated in the CSV)
        let commit_hash = raw.list_ancestors(tip_commit_hash, Some(1)).await?[0];
        if commit_hash == lfi.commit_hash {
            return Ok(Err("the received commit is already finalized.".to_owned()));
        }

        // Read the commits in the branch and verify them
        let commits = match read_commits(raw, lfi.commit_hash, commit_hash).await {
            Ok(x) => x,
            Err(CommitError::Commit(error, commit)) => {
                return Ok(Err(format!("failed to parse commit {commit}: {error}")));
            }
            Err(e) => return Err(e.into()),
        };
        for (commit, commit_hash) in &commits {
            if let Err(e) = csv.apply_commit(commit) {
                return Ok(Err(format!(
                    "commit sequence verification failed: {e} at {commit_hash}",
                )));
            }
        }

        let (last_commit, last_commit_hash) = commits.last().expect(
            "already checked that the received commit is not same as the last finalized block",
        );
        if let Commit::Block(_) = last_commit {
            if csv
                .verify_last_header_finalization(&last_finalization_proof.proof)
                .is_err()
            {
                return Ok(Err(
                    "finalization proof is invalid for the last block.".to_owned()
                ));
            }
            advance_finalized_branch(raw, *last_commit_hash, last_finalization_proof).await?;
        } else {
            return Ok(Err("fp commit must be on top of a block commit.".to_owned()));
        }
    }
    // If the branch ends with a block, agenda, or agenda proof commit
    else {
        if tip_commit_hash == lfi.commit_hash {
            return Ok(Err("the received commit is already finalized.".to_owned()));
        }

        // Read the commits in the branch and verify them
        let commits = match read_commits(raw, lfi.commit_hash, tip_commit_hash).await {
            Ok(x) => x,
            Err(CommitError::Commit(error, commit)) => {
                return Ok(Err(format!("failed to parse commit {commit}: {error}",)));
            }
            Err(e) => return Err(e.into()),
        };
        for (commit, commit_hash) in &commits {
            if let Err(e) = csv.apply_commit(commit) {
                return Ok(Err(format!(
                    "commit sequence verification failed: {e} at {commit_hash}",
                )));
            }
        }

        // If the commit sequence contains block commit(s) that can be finalized
        let headers = csv.get_block_headers();
        if headers.len() > 2 {
            let (last_header, _) = headers.last().expect(
                "already checked that the received commit is not same as the last finalized block",
            );
            let (second_to_last_header, index) = headers[headers.len() - 2].clone();
            advance_finalized_branch(
                raw,
                commits[index].1,
                LastFinalizationProof {
                    height: second_to_last_header.height,
                    proof: last_header.prev_block_finalization_proof.clone(),
                },
            )
            .await?;
        }

        // Create a branch associated to the last commit
        let branch_name = match &commits
            .last()
            .expect("already checked that the received commit is not a finalization proof commit")
            .0
        {
            Commit::Agenda(agenda) => {
                let approved_agendas = read::read_governance_approved_agendas(raw).await?;

                for (commit_hash, _) in approved_agendas {
                    if let Commit::AgendaProof(agenda_proof) =
                        read::read_commit(raw, commit_hash).await?
                    {
                        if agenda_proof.agenda_hash == agenda.to_hash256() {
                            return Ok(Err("agenda proof already exists.".to_owned()));
                        }
                    } else {
                        return Err(eyre!(IntegrityError::new(format!(
                            "commit {} is not an agenda proof",
                            commit_hash
                        ))));
                    }
                }

                format!(
                    "a-{}",
                    &agenda.to_hash256().to_string()[0..BRANCH_NAME_HASH_DIGITS]
                )
            }
            Commit::AgendaProof(agenda_proof) => {
                let agenda_name = match &commits[commits.len() - 2].0 {
                    Commit::Agenda(agenda) => {
                        format!(
                            "a-{}",
                            &agenda.to_hash256().to_string()[0..BRANCH_NAME_HASH_DIGITS]
                        )
                    }
                    _ => {
                        return Err(eyre!(IntegrityError::new(
                            "agenda proof's parent is not an agenda".to_string()
                        )))
                    }
                };
                if raw.locate_branch(agenda_name.clone()).await.is_ok() {
                    raw.delete_branch(agenda_name).await?;
                }

                format!(
                    "a-{}",
                    &agenda_proof.to_hash256().to_string()[0..BRANCH_NAME_HASH_DIGITS]
                )
            }
            Commit::Block(block) => {
                format!(
                    "b-{}",
                    &block.to_hash256().to_string()[0..BRANCH_NAME_HASH_DIGITS]
                )
            }
            x => return Ok(Err(format!("commit sequence ends with: {x:?}"))),
        };
        if raw.locate_branch(branch_name.clone()).await.is_ok() {
            return Ok(Err(format!("branch already exists: {branch_name}",)));
        }
        raw.create_branch(branch_name, tip_commit_hash).await?;
    };
    Ok(Ok(()))
}

pub async fn sync_all(raw: &mut RawRepository) -> Result<Vec<(String, Result<(), String>)>, Error> {
    let local_branches: Vec<String> = raw
        .list_branches()
        .await?
        .into_iter()
        .filter(|s| {
            s.as_str() != FINALIZED_BRANCH_NAME
                && s.as_str() != FP_BRANCH_NAME
                && !s.starts_with("a-")
                && !s.starts_with("b-")
                && s.as_str() != "p"
        })
        .collect();
    let remote_tracking_branches = raw.list_remote_tracking_branches().await?;

    let mut result = Vec::new();
    for branch in local_branches {
        result.push((
            branch.to_owned(),
            sync(raw, raw.locate_branch(branch.to_owned()).await?).await?,
        ));
    }
    for (remote, branch, commit_hash) in remote_tracking_branches {
        result.push((format!("{remote}/{branch}"), sync(raw, commit_hash).await?));
    }
    Ok(result)
}

pub async fn clean(raw: &mut RawRepository, hard: bool) -> Result<(), Error> {
    let finalized_branch_commit_hash = raw
        .locate_branch(FINALIZED_BRANCH_NAME.into())
        .await
        .map_err(|e| match e {
            raw::Error::NotFound(_) => {
                eyre!(IntegrityError::new(
                    "cannot locate `finalized` branch".to_string()
                ))
            }
            _ => eyre!(e),
        })?;
    let branches = read_local_branches(raw).await?;
    let last_header = read_last_finalized_block_header(raw).await?;
    for (branch, branch_commit_hash) in branches {
        if !(branch.as_str() == FINALIZED_BRANCH_NAME || branch.as_str() == FP_BRANCH_NAME) {
            if hard {
                raw.delete_branch(branch.to_string()).await?;
            } else {
                // Delete outdated branch
                let find_merge_base_result = raw
                    .find_merge_base(branch_commit_hash, finalized_branch_commit_hash)
                    .await
                    .map_err(|e| match e {
                        raw::Error::NotFound(_) => {
                            eyre!(IntegrityError::new(format!(
                                "cannot find merge base for branch {branch} and finalized branch"
                            )))
                        }
                        _ => eyre!(e),
                    })?;

                if finalized_branch_commit_hash != find_merge_base_result {
                    raw.delete_branch(branch.to_string()).await?;
                }

                // Delete branch with invalid commit sequence
                raw.checkout(branch.to_string()).await?;
                let reserved_state = raw.read_reserved_state().await?;
                let commits =
                    read_commits(raw, finalized_branch_commit_hash, branch_commit_hash).await?;
                let mut verifier =
                    CommitSequenceVerifier::new(last_header.clone(), reserved_state.clone())
                        .map_err(|e| eyre!("failed to create a commit sequence verifier: {}", e))?;
                for (commit, _) in commits.iter() {
                    if verifier.apply_commit(commit).is_err() {
                        raw.delete_branch(branch.to_string()).await?;
                    }
                }
            }
        }
    }

    // Remove remote repositories
    // Note that remote branches are automatically removed when the remote repository is removed.
    let remote_list = raw.list_remotes().await?;
    for (remote_name, _) in remote_list {
        raw.remove_remote(remote_name).await?;
    }

    Ok(())
}

pub async fn sync_old(
    raw: &mut RawRepository,
    block_hash: &Hash256,
    last_block_proof: &FinalizationProof,
) -> Result<(), Error> {
    let block_branch_name = format!("b-{}", &block_hash.to_string()[0..BRANCH_NAME_HASH_DIGITS]);
    let block_commit_hash = raw.locate_branch(block_branch_name.clone()).await?;

    if block_commit_hash == raw.locate_branch(FINALIZED_BRANCH_NAME.to_owned()).await? {
        info!("already finalized");
        return Ok(());
    }

    // Check if the last commit is a block commit.
    let current_finalized_commit = raw.locate_branch(FINALIZED_BRANCH_NAME.into()).await?;
    let new_commits = read_commits(raw, current_finalized_commit, block_commit_hash).await?;
    let last_block_header = if let Commit::Block(last_block_header) = &new_commits.last().unwrap().0
    {
        last_block_header
    } else {
        return Err(eyre!("the last commit is not a block commit"));
    };

    // Check if the given block commit is a descendant of the current finalized branch

    let find_merge_base_result = raw
        .find_merge_base(current_finalized_commit, block_commit_hash)
        .await
        .map_err(|e| match e {
            raw::Error::NotFound(_) => {
                eyre!(IntegrityError::new(format!(
                    "cannot find merge base for branch {block_branch_name} and finalized branch"
                )))
            }
            _ => eyre!(e),
        })?;
    if current_finalized_commit != find_merge_base_result {
        return Err(eyre!(
            "block commit is not a descendant of the current finalized branch"
        ));
    }

    // Verify every commit along the way.
    let last_finalized_block_header = read_last_finalized_block_header(raw).await?;
    let reserved_state = read_last_finalized_reserved_state(raw).await?;
    let mut verifier =
        CommitSequenceVerifier::new(last_finalized_block_header.clone(), reserved_state.clone())
            .map_err(|e| eyre!("failed to create a commit sequence verifier: {}", e))?;
    for (new_commit, new_commit_hash) in &new_commits {
        verifier
            .apply_commit(new_commit)
            .map_err(|e| eyre!("verification error on commit {}: {}", new_commit_hash, e))?;
    }
    verifier
        .verify_last_header_finalization(last_block_proof)
        .map_err(|e| eyre!("verification error on the last block header: {}", e))?;

    // If commit sequence verification is done and the finalization proof is verified,
    // move the `finalized` branch to the given block commit hash.
    // Then we update the `fp` branch.
    raw.checkout_clean().await?;
    raw.move_branch(FINALIZED_BRANCH_NAME.to_string(), block_commit_hash)
        .await?;
    raw.move_branch(FP_BRANCH_NAME.to_string(), block_commit_hash)
        .await?;
    raw.checkout(FP_BRANCH_NAME.into())
        .await
        .map_err(|e| match e {
            raw::Error::NotFound(_) => {
                eyre!(IntegrityError::new(format!(
                    "failed to checkout to the fp branch: {e}"
                )))
            }
            _ => eyre!(e),
        })?;
    raw.create_semantic_commit(format::fp_to_semantic_commit(&LastFinalizationProof {
        height: last_block_header.height,
        proof: last_block_proof.clone(),
    }))
    .await?;
    Ok(())
}
