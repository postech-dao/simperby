use super::*;
use log::trace;
use utils::*;

pub async fn fetch<T: RawRepository>(this: &mut DistributedRepository<T>) -> Result<(), Error> {
    add_remotes(this, &this.peers.read().await).await?;
    // TODO: handle this
    let _ = this.raw.fetch_all().await;

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

    // Step 0: try finalization with already tracked `b-` branches
    'branch_loop: for branch in this.raw.list_branches().await? {
        if branch.as_str().starts_with("b-") {
            let branch_commit_hash = this.raw.locate_branch(branch.clone()).await?;
            let header =
                if let Commit::Block(header) = read_commit(this, branch_commit_hash).await? {
                    header
                } else {
                    panic!("b- branch is not a block");
                };
            for proof in &finalization_proofs {
                if verify::verify_finalization_proof(&header, &proof.proof).is_ok() {
                    last_finalization_proof = proof.clone();
                    last_finalized_commit_hash = branch_commit_hash;
                    break 'branch_loop;
                }
            }
        }
    }

    // Step 1: update finalization
    'branch_loop: for (remote_name, branch_name, commit_hash) in remote_branches {
        let branch_displayed = format!(
            "{}/{}(at {})",
            remote_name,
            branch_name,
            serde_spb::to_string(&commit_hash).unwrap()
        );

        // Skip if the branch is `fp` or `work`.
        if branch_name == FP_BRANCH_NAME || branch_name == WORK_BRANCH_NAME {
            continue;
        }

        // Skip if the branch is already finalized by this node.
        if commit_hash == last_finalized_commit_hash {
            continue;
        }

        // Check if the branch is already in the local repo.
        if local_branches
            .iter()
            .any(|(_, local_branch_commit_hash)| *local_branch_commit_hash == commit_hash)
        {
            trace!(
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
            trace!("remote tracking branch outdated: {}", branch_displayed);
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

        // If this branch provided any other block commits
        if csv.get_block_headers().len() > 1 {
            // Store the block commits (including the currently finalized one)
            let block_commit_hashes =
                std::iter::once(last_finalized_commit_hash)
                    .chain(commits.iter().filter_map(|(commit, hash)| {
                        matches!(commit, Commit::Block(_)).then(|| *hash)
                    }))
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

            // If failed, finalize the second block from the last block
            // using `prev_block_finalization_proof`.
            if !hit && block_commit_hashes.len() > 2 {
                let last_block = csv.get_block_headers().last().unwrap().clone();
                last_finalization_proof = LastFinalizationProof {
                    proof: last_block.prev_block_finalization_proof.clone(),
                    height: last_block.height - 1,
                };
                last_finalized_commit_hash = block_commit_hashes[block_commit_hashes.len() - 2];
            }
        }
    }

    // Step 2: create `a` and `b` branches.
    for (remote_name, branch_name, commit_hash, commit) in next_step_branches {
        let branch_displayed = format!("{}/{}(at {})", remote_name, branch_name, commit_hash);
        match &commit {
            Commit::Agenda(agenda) => {
                if agenda.height == last_finalization_proof.height + 1 {
                    let calculated_branch_name = format!(
                        "a-{}",
                        &agenda.to_hash256().to_string()[0..BRANCH_NAME_HASH_DIGITS]
                    );
                    if calculated_branch_name != branch_name {
                        warn!(
                            "agenda branch name mismatch {}: should be {}",
                            branch_displayed, calculated_branch_name
                        );
                        continue;
                    }
                    this.raw.create_branch(branch_name, commit_hash).await?;
                }
            }
            Commit::AgendaProof(agenda_proof) => {
                if agenda_proof.height == last_finalization_proof.height + 1 {
                    let calculated_branch_name = format!(
                        "a-{}",
                        &agenda_proof.to_hash256().to_string()[0..BRANCH_NAME_HASH_DIGITS]
                    );
                    if calculated_branch_name != branch_name {
                        warn!(
                            "agenda-proof branch name mismatch {}: should be {}",
                            branch_displayed, calculated_branch_name
                        );
                        continue;
                    }
                    this.raw.create_branch(branch_name, commit_hash).await?;
                }
            }
            Commit::Block(block_header) => {
                if block_header.height == last_finalization_proof.height + 1 {
                    let calculated_branch_name = format!(
                        "b-{}",
                        &block_header.to_hash256().to_string()[0..BRANCH_NAME_HASH_DIGITS]
                    );
                    if calculated_branch_name != branch_name {
                        warn!(
                            "block branch name mismatch {}: should be {}",
                            branch_displayed, calculated_branch_name
                        );
                        continue;
                    }
                    this.raw.create_branch(branch_name, commit_hash).await?;
                }
            }
            x => warn!("incomplete remote branch {}: got {:?}", branch_displayed, x),
        }
    }

    // Step 3: apply the final finalization result
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
