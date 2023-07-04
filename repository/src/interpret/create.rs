use super::*;
use read::*;

pub async fn approve(
    raw: &mut RawRepository,
    agenda_hash: &Hash256,
    proof: Vec<TypedSignature<Agenda>>,
    timestamp: Timestamp,
) -> Result<CommitHash, Error> {
    let approved_agendas = read::read_governance_approved_agendas(raw).await?;

    for (commit_hash, _) in approved_agendas {
        if let Commit::AgendaProof(agenda_proof) = read::read_commit(raw, commit_hash).await? {
            if agenda_proof.agenda_hash == *agenda_hash {
                // already approved
                return Ok(commit_hash);
            }
        } else {
            return Err(eyre!(IntegrityError::new(format!(
                "commit {} is not an agenda proof",
                commit_hash
            ))));
        }
    }

    // Check if the agenda branch is rebased on top of the `finalized` branch.
    let last_header_commit = raw.locate_branch(FINALIZED_BRANCH_NAME.into()).await?;
    let agenda_branch_name = format!("a-{}", &agenda_hash.to_string()[0..BRANCH_NAME_HASH_DIGITS]);
    let agenda_commit_hash = raw.locate_branch(agenda_branch_name.clone()).await?;
    let find_merge_base_result = raw
        .find_merge_base(last_header_commit, agenda_commit_hash)
        .await
        .map_err(|e| match e {
            raw::Error::NotFound(_) => {
                eyre!(IntegrityError::new(format!(
                    "cannot find merge base for branch {agenda_branch_name} and finalized branch"
                )))
            }
            _ => eyre!(e),
        })?;

    if last_header_commit != find_merge_base_result {
        return Err(eyre!(
            "branch {} should be rebased on {}",
            agenda_branch_name,
            FINALIZED_BRANCH_NAME
        ));
    }

    // Verify all the incoming commits
    let finalized_header = read_last_finalized_block_header(raw).await?;
    let reserved_state = read_last_finalized_reserved_state(raw).await?;
    let finalized_commit_hash = raw.locate_branch(FINALIZED_BRANCH_NAME.into()).await?;
    let commits = read_commits(raw, finalized_commit_hash, agenda_commit_hash).await?;
    let mut verifier =
        CommitSequenceVerifier::new(finalized_header.clone(), reserved_state.clone())
            .map_err(|e| eyre!("failed to create a commit sequence verifier: {}", e))?;
    for (commit, hash) in commits.iter() {
        verifier
            .apply_commit(commit)
            .map_err(|e| eyre!("verification error on commit {}: {}", hash, e))?;
    }
    // Verify agenda with agenda proof
    let agenda_commit = commits.iter().map(|(commit, _)| commit).last().unwrap();
    let agenda = match agenda_commit {
        Commit::Agenda(agenda) => agenda,
        _ => return Err(eyre::eyre!("not an agenda commit")),
    };
    // Delete past `a-(trimmed agenda hash)` branch and create new `a-(trimmed agenda proof hash)` branch
    raw.delete_branch(agenda_branch_name.clone()).await?;
    // Create agenda proof commit
    let agenda_proof = AgendaProof {
        height: agenda.height,
        agenda_hash: agenda_commit.to_hash256(),
        proof,
        timestamp,
    };

    let agenda_proof_commit = Commit::AgendaProof(agenda_proof.clone());
    let agenda_proof_semantic_commit =
        format::to_semantic_commit(&agenda_proof_commit, reserved_state)?;
    let agenda_proof_branch_name = format!(
        "a-{}",
        &agenda_proof_commit.to_hash256().to_string()[0..BRANCH_NAME_HASH_DIGITS]
    );
    // Check if it is already approved.
    if raw
        .list_branches()
        .await?
        .contains(&agenda_proof_branch_name)
    {
        return Ok(raw.locate_branch(agenda_proof_branch_name.clone()).await?);
    }
    raw.create_branch(agenda_proof_branch_name.clone(), agenda_commit_hash)
        .await?;
    raw.checkout(agenda_proof_branch_name).await?;
    let agenda_proof_commit_hash = raw
        .create_semantic_commit(agenda_proof_semantic_commit)
        .await?;

    Ok(agenda_proof_commit_hash)
}

pub async fn create_agenda(
    raw: &mut RawRepository,
    author: MemberName,
) -> Result<(Agenda, CommitHash), Error> {
    let last_header = read_last_finalized_block_header(raw).await?;
    raw.check_clean()
        .await
        .map_err(|e| eyre!("repository is not clean: {e}"))?;
    let head = raw.get_head().await?;
    let last_header_commit = raw.locate_branch(FINALIZED_BRANCH_NAME.into()).await?;

    // Check if HEAD is rebased on top of the `finalized` branch.
    if raw.find_merge_base(last_header_commit, head).await? != last_header_commit {
        return Err(eyre!("HEAD should be rebased on {}", FINALIZED_BRANCH_NAME));
    }
    // Check the validity of the commit sequence
    let reserved_state = read_last_finalized_reserved_state(raw).await?;
    let mut verifier = CommitSequenceVerifier::new(last_header.clone(), reserved_state.clone())
        .map_err(|e| eyre!("failed to create a commit sequence verifier: {}", e))?;
    let commits = read_commits(raw, last_header_commit, head).await?;
    for (commit, hash) in commits.iter() {
        verifier
            .apply_commit(commit)
            .map_err(|e| eyre!("verification error on commit {}: {}", hash, e))?;
    }

    // Create agenda commit
    let mut transactions = Vec::new();
    for (commit, _) in commits {
        if let Commit::Transaction(t) = commit {
            transactions.push(t.clone());
        }
    }
    let agenda = Agenda {
        author,
        timestamp: get_timestamp(),
        transactions_hash: Agenda::calculate_transactions_hash(&transactions),
        height: last_header.height + 1,
        previous_block_hash: last_header.to_hash256(),
    };
    let agenda_commit = Commit::Agenda(agenda.clone());
    verifier.apply_commit(&agenda_commit).map_err(|_| {
        eyre!("agenda commit cannot be created on top of the current commit sequence")
    })?;

    let semantic_commit = to_semantic_commit(&agenda_commit, reserved_state)?;

    raw.checkout_clean().await?;
    let result = raw.create_semantic_commit(semantic_commit).await?;
    let mut agenda_branch_name = agenda_commit.to_hash256().to_string();
    agenda_branch_name.truncate(BRANCH_NAME_HASH_DIGITS);
    let agenda_branch_name = format!("a-{agenda_branch_name}");
    raw.create_branch(agenda_branch_name, result).await?;
    Ok((agenda, result))
}

pub async fn create_block(
    raw: &mut RawRepository,
    author: PublicKey,
) -> Result<(BlockHeader, CommitHash), Error> {
    raw.check_clean()
        .await
        .map_err(|e| eyre!("repository is not clean: {e}"))?;
    let head = raw.get_head().await?;
    let last_header_commit = raw.locate_branch(FINALIZED_BRANCH_NAME.into()).await?;

    // Check if HEAD branch is rebased on top of the `finalized` branch.
    if raw.find_merge_base(last_header_commit, head).await? != last_header_commit {
        return Err(eyre!("HEAD should be rebased on {}", FINALIZED_BRANCH_NAME));
    }

    // Check the validity of the commit sequence
    let commits = read_commits(raw, last_header_commit, head).await?;
    let last_header = read_last_finalized_block_header(raw).await?;
    let reserved_state = read_last_finalized_reserved_state(raw).await?;
    let mut verifier = CommitSequenceVerifier::new(last_header.clone(), reserved_state.clone())
        .map_err(|e| eyre!("failed to create a commit sequence verifier: {}", e))?;
    for (commit, hash) in commits.iter() {
        verifier
            .apply_commit(commit)
            .map_err(|e| eyre!("verification error on commit {}: {}", hash, e))?;
    }

    // Verify `finalization_proof`
    let fp_commit_hash = raw.locate_branch(FP_BRANCH_NAME.into()).await?;
    let fp_semantic_commit = raw.read_semantic_commit(fp_commit_hash).await?;
    let finalization_proof = fp_from_semantic_commit(fp_semantic_commit).unwrap().proof;

    // Create block commit
    let block_header = BlockHeader {
        author: author.clone(),
        prev_block_finalization_proof: finalization_proof,
        previous_hash: last_header.to_hash256(),
        height: last_header.height + 1,
        timestamp: get_timestamp(),
        commit_merkle_root: BlockHeader::calculate_commit_merkle_root(
            &commits
                .iter()
                .map(|(commit, _)| commit.clone())
                .collect::<Vec<_>>(),
        ),
        repository_merkle_root: Hash256::zero(), // TODO
        validator_set: reserved_state.get_validator_set().unwrap(),
        version: SIMPERBY_CORE_PROTOCOL_VERSION.to_string(),
    };
    let block_commit = Commit::Block(block_header.clone());
    verifier.apply_commit(&block_commit).map_err(|e| {
        eyre!("block commit cannot be created on top of the current commit sequence: {e}")
    })?;

    let semantic_commit = to_semantic_commit(&block_commit, reserved_state)?;

    raw.checkout_clean().await?;
    raw.checkout_detach(head).await?;
    let result = raw.create_semantic_commit(semantic_commit).await?;
    let mut block_branch_name = block_commit.to_hash256().to_string();
    block_branch_name.truncate(BRANCH_NAME_HASH_DIGITS);
    let block_branch_name = format!("b-{block_branch_name}");
    raw.create_branch(block_branch_name.clone(), result).await?;
    raw.checkout(block_branch_name).await?;
    Ok((block_header, result))
}

pub async fn create_extra_agenda_transaction(
    raw: &mut RawRepository,
    transaction: &ExtraAgendaTransaction,
) -> Result<CommitHash, Error> {
    raw.check_clean()
        .await
        .map_err(|e| eyre!("repository is not clean: {e}"))?;
    let head = raw.get_head().await?;
    let last_header_commit = raw.locate_branch(FINALIZED_BRANCH_NAME.into()).await?;
    let reserved_state = read_last_finalized_reserved_state(raw).await?;

    // Check if HEAD branch is rebased on top of the `finalized` branch.
    if raw.find_merge_base(last_header_commit, head).await? != last_header_commit {
        return Err(eyre!("HEAD should be rebased on {}", FINALIZED_BRANCH_NAME));
    }

    // Check the validity of the commit sequence
    let commits = read_commits(raw, last_header_commit, head).await?;
    let last_header = read_last_finalized_block_header(raw).await?;
    let mut verifier = CommitSequenceVerifier::new(last_header.clone(), reserved_state.clone())
        .map_err(|e| eyre!("failed to create a commit sequence verifier: {}", e))?;
    for (commit, hash) in commits.iter() {
        verifier
            .apply_commit(commit)
            .map_err(|e| eyre!("verification error on commit {}: {}", hash, e))?;
    }

    let extra_agenda_tx_commit = Commit::ExtraAgendaTransaction(transaction.clone());
    verifier.apply_commit(&extra_agenda_tx_commit).map_err(|_| {
            eyre!(
                "extra-agenda transaction commit cannot be created on top of the current commit sequence"
            )
        })?;

    let semantic_commit = to_semantic_commit(&extra_agenda_tx_commit, reserved_state)?;

    raw.checkout_clean().await?;
    let result = raw.create_semantic_commit(semantic_commit).await?;
    Ok(result)
}

pub async fn finalize(
    raw: &mut RawRepository,
    block_commit_hash: CommitHash,
    proof: FinalizationProof,
) -> Result<CommitHash, Error> {
    let csv = read_and_verify_commits_from_last_finalized_block(raw, block_commit_hash).await??;
    if let Commit::Block(block) = csv
        .get_total_commits()
        .last()
        .expect("there must be at least one commit in CSV")
    {
        csv.verify_last_header_finalization(&proof)?;
        raw.checkout_clean().await?;
        raw.move_branch(FP_BRANCH_NAME.to_string(), block_commit_hash)
            .await
            .map_err(|e| match e {
                raw::Error::NotFound(_) => {
                    eyre!(IntegrityError::new(format!(
                        "failed to find fp branch: {e}"
                    )))
                }
                _ => eyre!(e),
            })?;
        raw.checkout(FP_BRANCH_NAME.into())
            .await
            .map_err(|e| match e {
                raw::Error::NotFound(_) => {
                    eyre!(IntegrityError::new(format!(
                        "failed to find the fp branch: {e}"
                    )))
                }
                _ => eyre!(e),
            })?;
        let commit_hash = raw
            .create_semantic_commit(format::fp_to_semantic_commit(&LastFinalizationProof {
                height: block.height,
                proof,
            }))
            .await?;
        sync(raw, commit_hash)
            .await?
            .expect("already checked by CSV");
        Ok(commit_hash)
    } else {
        Err(eyre!("commit {} is not a block commit", block_commit_hash))
    }
}

pub async fn commit_gitignore(raw: &mut RawRepository) -> Result<(), Error> {
    raw.check_clean().await?;
    if check_gitignore(raw).await? {
        return Err(eyre!(".simperby/ entry already exists in .gitignore"));
    }
    let path = raw.get_working_directory_path().await?;
    let path = std::path::Path::new(&path).join(".gitignore");
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    file.write_all(b".simperby/\n").await?;

    let commit = RawCommit {
        message: "Add `.simperby/` entry to .gitignore".to_string(),
        diff: None,
        author: "Simperby".to_string(),
        email: "hi@simperby.net".to_string(),
        timestamp: get_timestamp() / 1000,
    };
    raw.create_commit_all(commit).await?;
    Ok(())
}
