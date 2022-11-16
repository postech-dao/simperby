use crate::*;

impl ToHash256 for String {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(self.as_bytes())
    }
}

impl ToHash256 for Member {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_json::to_vec(self).unwrap())
    }
}

impl ToHash256 for BlockHeader {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_json::to_vec(self).unwrap())
    }
}

impl ToHash256 for Diff {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_json::to_vec(self).unwrap())
    }
}

impl ToHash256 for Transaction {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_json::to_vec(self).unwrap())
    }
}

impl ToHash256 for Agenda {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_json::to_vec(self).unwrap())
    }
}

impl ToHash256 for AgendaProof {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_json::to_vec(self).unwrap())
    }
}

impl ToHash256 for ExtraAgendaTransaction {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_json::to_vec(self).unwrap())
    }
}

impl ToHash256 for ChatLog {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_json::to_vec(self).unwrap())
    }
}

impl ToHash256 for GenesisInfo {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_json::to_vec(self).unwrap())
    }
}

impl ToHash256 for Commit {
    fn to_hash256(&self) -> Hash256 {
        match self {
            Commit::Block(x) => x.to_hash256(),
            Commit::Transaction(x) => x.to_hash256(),
            Commit::Agenda(x) => x.to_hash256(),
            Commit::AgendaProof(x) => x.to_hash256(),
            Commit::ExtraAgendaTransaction(x) => x.to_hash256(),
            Commit::ChatLog(x) => x.to_hash256(),
        }
    }
}

impl Transaction {
    /// Returns the alternative hash of the transaction, which is for the Merkle tree.
    pub fn merkle_hash(&self) -> Hash256 {
        Hash256::hash(self.body.as_bytes())
    }
}

impl Agenda {
    pub fn calculate_hash(height: BlockHeight, transactions: &[Transaction]) -> Hash256 {
        let mut hash = Hash256::hash(format!("{}", height));
        for tx in transactions {
            hash = hash.aggregate(&tx.to_hash256());
        }
        hash
    }
}

impl BlockHeader {
    /// Calculates `commit_merkle_root`. Note that it doesn't verify the commits.
    pub fn calculate_commit_merkle_root(commits: &[Commit]) -> Hash256 {
        let merkle_tree = crate::merkle_tree::OneshotMerkleTree::create(
            commits.iter().map(|x| x.to_hash256()).collect(),
        );
        merkle_tree.root()
    }

    // note that `repository_merkle_root` is calculated from `simperby-repository`.
}
