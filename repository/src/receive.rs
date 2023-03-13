use super::*;
use utils::*;

async fn advance_finalized_branch(
    this: &mut DistributedRepository,
    to_be_finalized_block_commit_hash: CommitHash,
    finalization_proof: LastFinalizationProof,
) -> Result<(), Error> {
    this.raw.checkout_clean().await?;
    this.raw
        .move_branch(
            FINALIZED_BRANCH_NAME.into(),
            to_be_finalized_block_commit_hash,
        )
        .await?;
    this.raw
        .move_branch(FP_BRANCH_NAME.into(), to_be_finalized_block_commit_hash)
        .await?;
    this.raw.checkout(FP_BRANCH_NAME.into()).await?;
    this.raw
        .create_semantic_commit(format::fp_to_semantic_commit(&finalization_proof))
        .await?;
    Ok(())
}

/// Receives a new branch from a remote peer (either fetch or push).
///
/// - Returns `Ok(Ok(()))` if the branch is successfully received.
/// - Returns `Ok(Err(_))` if the branch is invalid and thus rejected, with the reason.
/// - Returns `Err(_)` if an error occurs.
pub async fn receive(
    this: &mut DistributedRepository,
    tip_commit_hash: CommitHash,
) -> Result<Result<(), String>, Error> {
    let last_finalized_block_header = this.get_last_finalized_block_header().await?;
    let last_finalized_commit_hash = this.raw.locate_branch(FINALIZED_BRANCH_NAME.into()).await?;
    let reserved_state = this.get_reserved_state().await?;
    let mut csv =
        CommitSequenceVerifier::new(last_finalized_block_header.clone(), reserved_state.clone())
            .map_err(|e| {
                IntegrityError::new(format!("finalized branch is not accepted by CSV: {e}"))
            })?;

    if this
        .raw
        .find_merge_base(last_finalized_commit_hash, tip_commit_hash)
        .await?
        != last_finalized_commit_hash
    {
        return Ok(Err(
            "the received branch tip commit is not a descendant of the last finalized block."
                .to_owned(),
        ));
    }

    // If the branch ends with a finalization proof commit
    if let Ok(last_finalization_proof) =
        format::fp_from_semantic_commit(this.raw.read_semantic_commit(tip_commit_hash).await?)
    {
        // Now we consider the direct parent as the received commit
        // (fp is not treated in the CSV)
        let commit_hash = this.raw.list_ancestors(tip_commit_hash, Some(1)).await?[0];
        if commit_hash == last_finalized_commit_hash {
            return Ok(Err("the received commit is already finalized.".to_owned()));
        }

        // Read the commits in the branch and verify them
        let commits = match read_commits(this, last_finalized_commit_hash, commit_hash).await {
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
            advance_finalized_branch(this, *last_commit_hash, last_finalization_proof).await?;
        } else {
            return Ok(Err("fp commit must be on top of a block commit.".to_owned()));
        }
    }
    // If the branch ends with a block, agenda, or agenda proof commit
    else {
        if tip_commit_hash == last_finalized_commit_hash {
            return Ok(Err("the received commit is already finalized.".to_owned()));
        }

        // Read the commits in the branch and verify them
        let commits = match read_commits(this, last_finalized_commit_hash, tip_commit_hash).await {
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
                this,
                commits[index].1,
                LastFinalizationProof {
                    height: second_to_last_header.height,
                    proof: last_header.prev_block_finalization_proof.clone(),
                },
            )
            .await?;
        }

        // Create the associate branch
        let branch_name = match &commits
            .last()
            .expect("already checked that the received commit is not a finalization proof commit")
            .0
        {
            Commit::Agenda(agenda) => {
                format!(
                    "a-{}",
                    &agenda.to_hash256().to_string()[0..BRANCH_NAME_HASH_DIGITS]
                )
            }
            Commit::AgendaProof(agenda_proof) => {
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
        if this.raw.locate_branch(branch_name.clone()).await.is_ok() {
            return Ok(Err(format!("branch already exists: {branch_name}",)));
        }
        this.raw.create_branch(branch_name, tip_commit_hash).await?;
    };
    Ok(Ok(()))
}
