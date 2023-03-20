pub mod format;
pub mod interpret;
pub mod raw;
// TODO: integrate the server feature with `DistributedRepository`
pub mod server;

use eyre::eyre;
use format::*;
use futures::prelude::*;
use interpret::*;
use log::info;
use raw::RawRepository;
use serde::{Deserialize, Serialize};
use simperby_core::reserved::ReservedState;
use simperby_core::utils::get_timestamp;
use simperby_core::verify::CommitSequenceVerifier;
use simperby_core::*;
use std::sync::Arc;
use std::{collections::HashSet, fmt};
use tokio::sync::RwLock;

pub type Branch = String;
pub type Tag = String;

pub const FINALIZED_BRANCH_NAME: &str = "finalized";
pub const WORK_BRANCH_NAME: &str = "work";
pub const FP_BRANCH_NAME: &str = "fp";
pub const COMMIT_TITLE_HASH_DIGITS: usize = 8;
pub const TAG_NAME_HASH_DIGITS: usize = 8;
pub const BRANCH_NAME_HASH_DIGITS: usize = 8;
pub const UNKNOWN_COMMIT_AUTHOR: &str = "unknown";

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash)]
pub struct CommitHash {
    pub hash: [u8; 20],
}

impl ToHash256 for CommitHash {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(self.hash)
    }
}

impl Serialize for CommitHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(hex::encode(self.hash).as_str())
    }
}

impl fmt::Display for CommitHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.hash).as_str())
    }
}

impl<'de> Deserialize<'de> for CommitHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let hash = hex::decode(s).map_err(serde::de::Error::custom)?;
        if hash.len() != 20 {
            return Err(serde::de::Error::custom("invalid length"));
        }
        let mut hash_array = [0; 20];
        hash_array.copy_from_slice(&hash);
        Ok(CommitHash { hash: hash_array })
    }
}

pub type Error = eyre::Error;

#[derive(thiserror::Error, Debug)]
#[error("repository integrity broken: {msg}")]
pub struct IntegrityError {
    pub msg: String,
}

impl IntegrityError {
    pub fn new(msg: String) -> Self {
        Self { msg }
    }
}

pub struct FinalizationInfo {
    pub header: BlockHeader,
    pub commit_hash: CommitHash,
    pub reserved_state: ReservedState,
    pub proof: FinalizationProof,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// The distance that if a commit is past this far,
    /// any forked branch starting from the commit
    /// will be considered a long range attack and thus ignored.
    ///
    /// If zero, fork can be detected only from the currently last-finalized commit.
    pub long_range_attack_distance: usize,
}

/// The local Simperby blockchain data repository.
///
/// It automatically locks the repository once created.
///
/// - It **verifies** all the incoming changes and applies them to the local repository
/// only if they are valid.
pub struct DistributedRepository {
    /// We keep the `RawRepository` in a `RwLock` for possible concurrent accesses in some operations.
    raw: Arc<RwLock<RawRepository>>,
    _config: Config,
    private_key: Option<PrivateKey>,
}

impl DistributedRepository {
    pub fn get_raw(&self) -> Arc<RwLock<RawRepository>> {
        Arc::clone(&self.raw)
    }

    pub async fn new(
        raw: Arc<RwLock<RawRepository>>,
        config: Config,
        private_key: Option<PrivateKey>,
    ) -> Result<Self, Error> {
        Ok(Self {
            raw,
            _config: config,
            private_key,
        })
    }

    /// Initializes the genesis repository, leaving a genesis header.
    ///
    /// It also
    /// - creates `fp` branch and its commit (for the genesis block).
    /// - creates `work` branch at the same place with the `finalized` branch.
    ///
    /// Note that `genesis` can be called on any commit except a merge commit.
    pub async fn genesis(&mut self) -> Result<(), Error> {
        genesis(&mut *self.raw.write().await).await
    }

    // ---------------
    // Read-only operations
    // ---------------

    /// Reads the last finalization information from the repository.
    pub async fn read_last_finalization_info(&self) -> Result<FinalizationInfo, Error> {
        read_last_finalization_info(&*self.raw.read().await).await
    }

    /// Reads the finalization information at specific height.
    pub async fn read_finalization_info(
        &self,
        _height: BlockHeight,
    ) -> Result<FinalizationInfo, Error> {
        todo!()
    }

    /// Reads the given commit.
    pub async fn read_commit(&self, commit_hash: CommitHash) -> Result<Commit, Error> {
        read_commit(&*self.raw.read().await, commit_hash).await
    }

    /// Returns the currently valid and height-acceptable agendas in the repository.
    pub async fn read_agendas(&self) -> Result<Vec<(CommitHash, Hash256)>, Error> {
        read_agendas(&*self.raw.read().await).await
    }

    /// Returns governance-approved agendas in the repository.
    /// The result will be a list of agenda proofs, not just agendas.
    pub async fn read_governance_approved_agendas(
        &self,
    ) -> Result<Vec<(CommitHash, Hash256)>, Error> {
        read_governance_approved_agendas(&*self.raw.read().await).await
    }

    /// Returns the currently valid and height-acceptable blocks in the repository.
    pub async fn read_blocks(&self) -> Result<Vec<(CommitHash, Hash256)>, Error> {
        read_blocks(&*self.raw.read().await).await
    }

    /// Checks the validity of the repository, starting from the given height.
    ///
    /// It checks
    /// 1. all the reserved branches and tags
    /// 2. the finalization proof in the `fp` branch.
    /// 3. the existence of merge commits
    /// 4. the canonical history of the `finalized` branch.
    /// 5. the reserved state in a valid format.
    pub async fn check(&self, _starting_height: BlockHeight) -> Result<bool, Error> {
        todo!()
    }

    // ---------------
    // Operations that interact with possible local works
    // (manually added commits or remote tracking branches)
    // ---------------

    /// Synchronizes the repository with the given commit (interpreted as a branch tip).
    /// - Returns `Ok(Ok(()))` if the branch is successfully received.
    /// - Returns `Ok(Err(_))` if the branch is invalid and thus rejected, with a reason.
    /// - Returns `Err(_)` if an error occurs.
    ///
    /// 1. Finalization: move the `finalized` and `fp` branch to the last finalized block commit.
    /// 2. Block observed: add a `b-#` branch on the block candidate.
    /// 3. Agenda observed (either governance-approved or not): add an `a-#` branch on the agenda candidate.
    ///
    /// This will verify every commit along the way.
    /// If the given commit is not a descendant of the
    /// current `finalized` (i.e., cannot be fast-forwarded), it fails.
    pub async fn sync(&mut self, commit_hash: CommitHash) -> Result<Result<(), String>, Error> {
        sync(&mut *self.raw.write().await, commit_hash).await
    }

    /// Performs `sync()` on all local branches and remote tracking branches on the repository.
    ///
    /// Returns the list of `(branch name, result of sync())`.
    pub async fn sync_all(&mut self) -> Result<Vec<(String, Result<(), String>)>, Error> {
        sync_all(&mut *self.raw.write().await).await
    }

    /// Tests if the given push request is acceptable.
    pub async fn test_push_eligibility(
        &self,
        commit_hash: CommitHash,
        branch_name: String,
        timestamp: Timestamp,
        signature: TypedSignature<(CommitHash, String, Timestamp)>,
        _timestamp_to_test: Timestamp,
    ) -> Result<bool, Error> {
        test_push_eligibility(
            &*self.raw.read().await,
            commit_hash,
            branch_name,
            timestamp,
            signature,
            _timestamp_to_test,
        )
        .await
    }

    /// Cleans all the outdated commits, remote repositories and branches.
    ///
    /// It will leave only
    /// - the `finalized` branch
    /// - the `work` branch
    /// - the `fp` branch
    /// when `hard` is `true`,
    ///
    /// and when `hard` is `false`,
    /// - the `p` branch
    /// - the `a-#` branches
    /// - the `b-#` branches
    /// will be left as well
    /// if only the branches have valid commit sequences
    /// and are not outdated (branched from the last finalized commit).
    pub async fn clean(&mut self, hard: bool) -> Result<(), Error> {
        clean(&mut *self.raw.write().await, hard).await
    }

    /// Broadcasts all the local messages.
    pub async fn broadcast(&mut self) -> Result<(), Error> {
        broadcast(&mut *self.raw.write().await, self.private_key.clone()).await
    }

    // ---------------
    // DMS-related operations
    // ---------------

    pub async fn flush(&mut self) -> Result<(), Error> {
        todo!()
    }

    pub async fn update(&mut self) -> Result<(), Error> {
        todo!()
    }

    // ---------------
    // Various operations that (might) create a commit
    // ---------------

    /// Informs that the given agenda has been approved.
    ///
    /// After verification, it will create an agenda-proof commit,
    /// and update the corresponding `a-#` branch to it
    pub async fn approve(
        &mut self,
        agenda_hash: &Hash256,
        proof: Vec<TypedSignature<Agenda>>,
        timestamp: Timestamp,
    ) -> Result<CommitHash, Error> {
        approve(&mut *self.raw.write().await, agenda_hash, proof, timestamp).await
    }

    /// Creates an agenda commit on top of the `work` branch.
    pub async fn create_agenda(
        &mut self,
        author: MemberName,
    ) -> Result<(Agenda, CommitHash), Error> {
        create_agenda(&mut *self.raw.write().await, author).await
    }

    /// Creates a block commit on top of the `work` branch.
    pub async fn create_block(
        &mut self,
        author: PublicKey,
    ) -> Result<(BlockHeader, CommitHash), Error> {
        create_block(&mut *self.raw.write().await, author).await
    }

    /// Creates an extra-agenda transaction commit on top of the `work` branch.
    pub async fn create_extra_agenda_transaction(
        &mut self,
        transaction: &ExtraAgendaTransaction,
    ) -> Result<CommitHash, Error> {
        create_extra_agenda_transaction(&mut *self.raw.write().await, transaction).await
    }

    /// Finalizes the block with the given proof. Returns the commit hash of the updated `fp` branch.
    pub async fn finalize(
        &mut self,
        block_commit_hash: CommitHash,
        proof: FinalizationProof,
    ) -> Result<CommitHash, Error> {
        finalize(&mut *self.raw.write().await, block_commit_hash, proof).await
    }

    // ---------------
    // Tag-related operations
    // ---------------

    /// Puts a 'vote' tag on the commit.
    pub async fn vote(&mut self, commit_hash: CommitHash) -> Result<(), Error> {
        vote(&mut *self.raw.write().await, commit_hash).await
    }

    /// Puts a 'veto' tag on the commit.
    pub async fn veto(&mut self, commit_hash: CommitHash) -> Result<(), Error> {
        veto(&mut *self.raw.write().await, commit_hash).await
    }
}
