use super::*;

pub async fn vote(raw: &mut RawRepository, commit_hash: CommitHash) -> Result<(), Error> {
    let semantic_commit = raw.read_semantic_commit(commit_hash).await?;
    let commit = format::from_semantic_commit(semantic_commit).map_err(|e| eyre!(e))?;
    // Check if the commit is an agenda commit.
    if let Commit::Agenda(_) = commit {
        let mut vote_tag_name = commit.to_hash256().to_string();
        vote_tag_name.truncate(TAG_NAME_HASH_DIGITS);
        let vote_tag_name = format!("vote-{vote_tag_name}");
        raw.create_tag(vote_tag_name, commit_hash).await?;
        Ok(())
    } else {
        Err(eyre!("commit {} is not an agenda commit", commit_hash))
    }
}

pub async fn veto(raw: &mut RawRepository, commit_hash: CommitHash) -> Result<(), Error> {
    let semantic_commit = raw.read_semantic_commit(commit_hash).await?;
    let commit = format::from_semantic_commit(semantic_commit).map_err(|e| eyre!(e))?;
    // Check if the commit is a block commit.
    if let Commit::Block(_) = commit {
        let mut veto_tag_name = commit.to_hash256().to_string();
        veto_tag_name.truncate(TAG_NAME_HASH_DIGITS);
        let veto_tag_name = format!("veto-{veto_tag_name}");
        raw.create_tag(veto_tag_name, commit_hash).await?;
        Ok(())
    } else {
        Err(eyre!("commit {} is not a block commit", commit_hash))
    }
}
