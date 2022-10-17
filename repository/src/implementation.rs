use super::*;
use format::*;
use futures::prelude::*;
use simperby_common::verify::CommitSequenceVerifier;

fn get_timestamp() -> Timestamp {
    let now = std::time::SystemTime::now();
    let since_the_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    since_the_epoch.as_millis() as Timestamp
}

pub struct DistributedRepositoryImpl<T> {
    raw: T,
}

#[async_trait]
impl<T: RawRepository + 'static> DistributedRepository<T> for DistributedRepositoryImpl<T> {
    async fn new(_raw: T) -> Result<Self, Error>
    where
        Self: Sized,
    {
        unimplemented!()
    }

    async fn genesis(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    async fn get_last_finalized_block_header(&self) -> Result<BlockHeader, Error> {
        unimplemented!()
    }

    async fn fetch(
        &mut self,
        _network_config: &NetworkConfig,
        _known_peers: &[Peer],
    ) -> Result<(), Error> {
        unimplemented!()
    }

    async fn notify_push(
        &mut self,
        _network_config: &NetworkConfig,
        _known_peers: &[Peer],
    ) -> Result<(), Error> {
        unimplemented!()
    }

    async fn serve(
        self,
        _network_config: &NetworkConfig,
        _peers: SharedKnownPeers,
    ) -> Result<tokio::task::JoinHandle<Result<(), Error>>, Error> {
        unimplemented!()
    }

    async fn check(&self, _starting_height: BlockHeight) -> Result<bool, Error> {
        unimplemented!()
    }

    async fn sync(&mut self, _block_commit: &CommitHash) -> Result<(), Error> {
        unimplemented!()
    }

    async fn get_agendas(&self) -> Result<Vec<(CommitHash, Hash256)>, Error> {
        unimplemented!()
    }

    async fn get_blocks(&self) -> Result<Vec<(CommitHash, Hash256)>, Error> {
        unimplemented!()
    }

    async fn finalize(
        &mut self,
        _block_commit_hash: &CommitHash,
        _proof: &FinalizationProof,
    ) -> Result<(), Error> {
        unimplemented!()
    }

    async fn approve(
        &mut self,
        _agenda_commit_hash: &CommitHash,
        _proof: Vec<(PublicKey, TypedSignature<Agenda>)>,
    ) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    async fn create_agenda(&mut self, author: PublicKey) -> Result<CommitHash, Error> {
        let last_header = self.get_last_finalized_block_header().await?;
        let work_commit = self.raw.locate_branch(&WORK_BRANCH_NAME.into()).await?;
        let last_header_commit = self
            .raw
            .locate_branch(&FINALIZED_BRANCH_NAME.into())
            .await?;

        // Check if the `work` branch is rebased on top of the `finalized` branch.
        if self
            .raw
            .find_merge_base(&last_header_commit, &work_commit)
            .await?
            != last_header_commit
        {
            return Err(Error::Unknown(format!(
                "branch {} should be rebased on {}",
                WORK_BRANCH_NAME, FINALIZED_BRANCH_NAME
            )));
        }

        // Fetch and convert commits
        let commits = self.raw.list_ancestors(&work_commit, Some(256)).await?;
        let position = commits
            .iter()
            .position(|c| *c == last_header_commit)
            .expect("TODO: handle the case where it exceeds the limit.");

        // commits starting from the very next one to the last finalized block.
        let commits = stream::iter(commits.iter().take(position).rev().cloned().map(|c| {
            let raw = &self.raw;
            async move { raw.read_semantic_commit(&c).await.map(|x| (x, c)) }
        }))
        .buffered(256)
        .collect::<Vec<_>>()
        .await;
        let commits = commits.into_iter().collect::<Result<Vec<_>, _>>()?;
        let commits = commits
            .into_iter()
            .map(|(commit, hash)| {
                from_semantic_commit(commit, &last_header)
                    .map_err(|e| (e, hash))
                    .map(|x| (x, hash))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|(error, hash)| Error::Format(hash, error))?;

        // Check the validity of the commit sequence
        let mut verifier = CommitSequenceVerifier::new(last_header.clone())
            .map_err(|e| Error::Verification(last_header_commit, e))?;
        for (commit, hash) in commits.iter() {
            verifier
                .apply_commit(commit)
                .map_err(|e| Error::Verification(*hash, e))?;
        }

        // Check whether the commit sequence is in the transaction phase.
        let mut transactions = Vec::new();

        for (commit, _) in commits {
            if let Commit::Transaction(t) = commit {
                transactions.push(t.clone());
            } else {
                return Err(Error::InvalidArgument(format!(
                    "branch {} is not in the transaction phase",
                    WORK_BRANCH_NAME
                )));
            }
        }

        let agenda_commit = Commit::Agenda(Agenda {
            author,
            timestamp: get_timestamp(),
            hash: Agenda::calculate_hash(last_header.height + 1, &transactions),
        });
        let semantic_commit = to_semantic_commit(&agenda_commit, &last_header);

        self.raw.checkout_clean().await?;
        self.raw.checkout(&FINALIZED_BRANCH_NAME.into()).await?;
        let result = self.raw.create_semantic_commit(semantic_commit).await?;
        Ok(result)
    }

    async fn create_block(&mut self, _author: PublicKey) -> Result<CommitHash, Error> {
        unimplemented!()
    }

    async fn create_extra_agenda_transaction(
        &mut self,
        _transaction: &ExtraAgendaTransaction,
    ) -> Result<CommitHash, Error> {
        unimplemented!()
    }
}
