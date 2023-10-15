use crate::{crypto::*, reserved::ReservedState};
use serde::{Deserialize, Serialize};
use std::fmt;

pub type VotingPower = u64;
/// A UNIX timestamp measured in milliseconds.
pub type Timestamp = i64;
/// A block height. The genesis block is at height 0.
pub type BlockHeight = u64;
pub type ConsensusRound = u64;
pub type MemberName = String;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Member {
    pub public_key: PublicKey,
    /// The name of the member that will be used in human-readable interfaces.
    /// This must be unique.
    pub name: MemberName,
    pub governance_voting_power: VotingPower,
    pub consensus_voting_power: VotingPower,
    /// If this member delegated its governance voting power to another member,
    /// the delegatee.
    pub governance_delegatee: Option<MemberName>,
    /// If this member delegated its governance consensus power to another member,
    /// the delegatee.
    pub consensus_delegatee: Option<MemberName>,
    /// If true, all voting powers are ignored.
    /// Note that once granted, Simperby keeps all members forever in the reserved state.
    /// If you want to remove a member, you must set this to true instead of removing the member.
    pub expelled: bool,
    // TODO: add various conditions for each delegation.
    // - Unlock-Automatically-After-N-Blocks
    // - Unlock-Automatically-After-T-Seconds
    // - Unlock-If-The-Delegatee-Is-Not-Active
    // - Unlock-If-The-Validator-Set-Changes
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct FinalizationSignTarget {
    pub block_hash: Hash256,
    pub round: ConsensusRound,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct FinalizationProof {
    pub round: ConsensusRound,
    pub signatures: Vec<TypedSignature<FinalizationSignTarget>>,
}

impl FinalizationProof {
    pub fn genesis() -> Self {
        FinalizationProof {
            round: 0,
            signatures: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct BlockHeader {
    /// The author of this block.
    pub author: PublicKey,
    /// The signature of the previous block.
    pub prev_block_finalization_proof: FinalizationProof,
    /// The hash of the previous block.
    pub previous_hash: Hash256,
    /// The height of this block.
    pub height: BlockHeight,
    /// The timestamp of this block.
    pub timestamp: Timestamp,
    /// The Merkle root of all the commits for this block.
    pub commit_merkle_root: Hash256,
    /// The Merkle root of the non-essential state.
    pub repository_merkle_root: Hash256,
    /// The effective validator set (delegation-applied) for the next block.
    ///
    /// The order here is the consensus leader selection order.
    pub validator_set: Vec<(PublicKey, VotingPower)>,
    /// The protocol version that must be used from next block.
    ///
    /// It must be a valid semantic version (e.g., `0.2.3`).
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Agenda {
    pub height: BlockHeight,
    pub author: MemberName,
    pub timestamp: Timestamp,
    pub transactions_hash: Hash256,
    pub previous_block_hash: Hash256,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct ChatLog {
    // TODO
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct AgendaProof {
    pub height: BlockHeight,
    pub agenda_hash: Hash256,
    pub proof: Vec<TypedSignature<Agenda>>,
    pub timestamp: Timestamp,
}

/// An abstracted diff of the state.
///
/// - The actual content of the diff (for the non-reserved state)
/// is not cared by the Simperby node. It only keeps the hash of it.
/// - It holds the reserved state as a `Box` to flatten the variant size.
/// (see https://rust-lang.github.io/rust-clippy/master/index.html#large_enum_variant)
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Diff {
    /// Nothing changed in the repository; an empty commit.
    None,
    /// Changes the reserved area ONLY.
    Reserved(Box<ReservedState>),
    /// Changes the non-reserved area ONLY.
    ///
    /// It contains the hash of the diff.
    NonReserved(Hash256),
    /// General diff that may change both the reserved state and the non-reserved state.
    General(Box<ReservedState>, Hash256),
}

/// A general transaction to be included in the agenda.
///
/// Note that none of the fields are checked by the Simperby core protocol;
/// they just represent a Git commit which is used for general data recording.
///
/// - `author` and `timestamp` is that of the **author signature** of the git commit.
/// - `committer` signature will be always the same as the `author` signature.
/// (if not, it will be rejected by the node)
/// - `head` and `body` might be used for the trustless message delivery.
/// Please refer to the *simperby-settlement* crate.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Transaction {
    pub author: MemberName,
    pub timestamp: Timestamp,
    pub head: String,
    pub body: String,
    pub diff: Diff,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ExtraAgendaTransaction {
    Delegate(TxDelegate),
    Undelegate(TxUndelegate),
    Report(TxReport),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct TxDelegate {
    pub data: DelegationTransactionData,
    pub proof: TypedSignature<DelegationTransactionData>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct TxUndelegate {
    pub data: UndelegationTransactionData,
    pub proof: TypedSignature<UndelegationTransactionData>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct TxReport {
    // TODO
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct DelegationTransactionData {
    pub delegator: MemberName,
    pub delegatee: MemberName,
    /// Whether to delegate the governance voting power too.
    pub governance: bool,
    pub block_height: BlockHeight,
    pub timestamp: Timestamp,
    pub chain_name: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct UndelegationTransactionData {
    pub delegator: MemberName,
    pub block_height: BlockHeight,
    pub timestamp: Timestamp,
    pub chain_name: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct GenesisInfo {
    pub header: BlockHeader,
    pub genesis_proof: FinalizationProof,
    pub chain_name: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Commit {
    Block(BlockHeader),
    Transaction(Transaction),
    Agenda(Agenda),
    AgendaProof(AgendaProof),
    ExtraAgendaTransaction(ExtraAgendaTransaction),
    ChatLog(ChatLog),
}

/// The special finalization proof commit in the `fp` branch.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct LastFinalizationProof {
    pub height: BlockHeight,
    pub proof: FinalizationProof,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash)]
pub struct CommitHash {
    pub hash: [u8; 20],
}

impl CommitHash {
    pub fn zero() -> Self {
        Self { hash: [0; 20] }
    }
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct FinalizationInfo {
    pub header: BlockHeader,
    pub commit_hash: CommitHash,
    pub reserved_state: ReservedState,
    pub proof: FinalizationProof,
}

#[cfg(test)]
mod tests {
    use super::CommitHash;
    use serde_json::{from_str, to_string};

    #[test]
    fn en_decode_commit_hash() {
        let commit_hash = CommitHash { hash: [1; 20] };
        let serialized = to_string(&commit_hash).unwrap();
        assert_eq!(serialized, "\"0101010101010101010101010101010101010101\"");
        let deserialized: CommitHash = from_str(&serialized).unwrap();
        assert_eq!(deserialized, commit_hash);
    }
}
