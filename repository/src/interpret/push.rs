use super::*;

pub async fn test_push_eligibility(
    raw: &RawRepository,
    commit_hash: CommitHash,
    branch_name: String,
    timestamp: Timestamp,
    signature: TypedSignature<(CommitHash, String, Timestamp)>,
    _timestamp_to_test: Timestamp,
) -> Result<bool, Error> {
    let reserved_state = raw.read_reserved_state().await?;
    // TODO: elaborate this to consider `Member::expelled`.
    let is_eligible = reserved_state
        .members
        .iter()
        .any(|member| member.public_key == *signature.signer());
    let is_eligible = is_eligible
        && signature
            .verify(&(commit_hash, branch_name, timestamp))
            .is_ok();
    // TODO: put the threshold in the config.
    // let is_eligible = is_eligible && (timestamp_to_test - timestamp).abs() <= 1000;
    Ok(is_eligible)
}

pub async fn broadcast(
    raw: &mut RawRepository,
    private_key: Option<PrivateKey>,
) -> Result<(), Error> {
    let agendas = read_agendas(raw).await?;
    let blocks = read_blocks(raw).await?;
    let remotes = raw.list_remotes().await?;

    for (remote_name, _) in remotes {
        for &(commit_hash, _) in &agendas {
            let timestamp = get_timestamp();
            let branch = &commit_hash
                .to_hash256()
                .aggregate(&timestamp.to_hash256())
                .to_string()[0..BRANCH_NAME_HASH_DIGITS];
            let signature = TypedSignature::sign(
                &(commit_hash, branch.to_owned(), timestamp as u64),
                private_key.as_ref().unwrap(),
            )?;
            let signer = serde_spb::to_string(signature.signer())?.replace('\"', "\\\"");
            let signature =
                serde_spb::to_string(&signature.get_raw_signature())?.replace('\"', "\\\"");

            raw.create_branch(branch.into(), commit_hash).await?;
            raw.push_option(
                remote_name.clone(),
                branch.into(),
                Some(format!(
                    "{commit_hash} {branch} {timestamp} {signature} {signer}"
                )),
            )
            .await?;
            raw.delete_branch(branch.into()).await?;
        }

        for &(commit_hash, _) in &blocks {
            let timestamp = get_timestamp();
            let branch = &commit_hash
                .to_hash256()
                .aggregate(&timestamp.to_hash256())
                .to_string()[0..BRANCH_NAME_HASH_DIGITS];
            let signature = TypedSignature::sign(
                &(commit_hash, branch.to_owned(), timestamp as u64),
                private_key.as_ref().unwrap(),
            )?;
            let signer = serde_spb::to_string(signature.signer())?.replace('\"', "\\\"");
            let signature =
                serde_spb::to_string(&signature.get_raw_signature())?.replace('\"', "\\\"");

            raw.create_branch(branch.into(), commit_hash).await?;
            raw.push_option(
                remote_name.clone(),
                branch.into(),
                Some(format!(
                    "{commit_hash} {branch} {timestamp} {signature} {signer}"
                )),
            )
            .await?;
            raw.delete_branch(branch.into()).await?;
        }

        // Push fp and finalized branch.
        let commit_hash = raw.locate_branch(FP_BRANCH_NAME.into()).await?;
        let timestamp = get_timestamp();
        let branch = &commit_hash
            .to_hash256()
            .aggregate(&timestamp.to_hash256())
            .to_string()[0..BRANCH_NAME_HASH_DIGITS];
        let signature = TypedSignature::sign(
            &(commit_hash, branch.to_owned(), timestamp as u64),
            private_key.as_ref().unwrap(),
        )?;
        let signer = serde_spb::to_string(signature.signer())?.replace('\"', "\\\"");
        let signature = serde_spb::to_string(&signature.get_raw_signature())?.replace('\"', "\\\"");

        raw.create_branch(branch.into(), commit_hash).await?;
        raw.push_option(
            remote_name.clone(),
            branch.into(),
            Some(format!(
                "{commit_hash} {branch} {timestamp} {signature} {signer}"
            )),
        )
        .await?;
        raw.delete_branch(branch.into()).await?;
    }
    Ok(())
}
