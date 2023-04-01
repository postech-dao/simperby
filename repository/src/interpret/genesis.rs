use super::*;

pub async fn genesis(raw: &mut RawRepository) -> Result<(), Error> {
    let reserved_state = raw.read_reserved_state().await?;
    let block_commit = Commit::Block(reserved_state.genesis_info.header.clone());
    let semantic_commit = to_semantic_commit(&block_commit, reserved_state.clone())?;

    raw.checkout_clean().await?;
    // TODO: ignore only if the error is 'already exists'. Otherwise, propagate the error.
    let _ = raw
        .create_branch(FINALIZED_BRANCH_NAME.into(), raw.get_head().await?)
        .await;
    raw.checkout(FINALIZED_BRANCH_NAME.into())
        .await
        .map_err(|e| match e {
            raw::Error::NotFound(_) => {
                eyre!(IntegrityError::new(format!(
                    "failed to checkout to the finalized branch: {e}"
                )))
            }
            _ => eyre!(e),
        })?;
    let result = raw.create_semantic_commit(semantic_commit).await?;
    // TODO: ignore only if the error is 'already exists'. Otherwise, propagate the error.
    let _ = raw.create_branch(FP_BRANCH_NAME.into(), result).await;
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
    raw.create_semantic_commit(fp_to_semantic_commit(&LastFinalizationProof {
        height: 0,
        proof: reserved_state.genesis_info.genesis_proof.clone(),
    }))
    .await?;

    match raw.locate_branch("main".to_owned()).await {
        Ok(_) => {
            raw.checkout_detach(raw.get_head().await?).await?;
            raw.move_branch(
                "main".to_owned(),
                raw.locate_branch(FINALIZED_BRANCH_NAME.to_owned()).await?,
            )
            .await?;
            raw.checkout("main".to_owned()).await?;
        }
        Err(raw::Error::NotFound(_)) => {
            raw.create_branch(
                "main".to_owned(),
                raw.locate_branch(FINALIZED_BRANCH_NAME.to_owned()).await?,
            )
            .await?;
            raw.checkout("main".to_owned()).await?;
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
