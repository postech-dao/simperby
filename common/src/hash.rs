use crate::*;

impl ToHash256 for String {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(self.as_bytes())
    }
}

impl ToHash256 for u64 {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(self.to_le_bytes())
    }
}

impl<T1, T2> ToHash256 for (T1, T2)
where
    T1: ToHash256,
    T2: ToHash256,
{
    fn to_hash256(&self) -> Hash256 {
        self.0.to_hash256().aggregate(&self.1.to_hash256())
    }
}

impl<T1, T2, T3> ToHash256 for (T1, T2, T3)
where
    T1: ToHash256,
    T2: ToHash256,
    T3: ToHash256,
{
    fn to_hash256(&self) -> Hash256 {
        self.0
            .to_hash256()
            .aggregate(&self.1.to_hash256())
            .aggregate(&self.2.to_hash256())
    }
}

impl ToHash256 for Member {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

impl ToHash256 for FinalizationSignTarget {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

impl ToHash256 for BlockHeader {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

impl ToHash256 for Diff {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

impl ToHash256 for Transaction {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

impl ToHash256 for Agenda {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

impl ToHash256 for AgendaProof {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

impl ToHash256 for ExtraAgendaTransaction {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

impl ToHash256 for DelegationTransactionData {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

impl ToHash256 for UndelegationTransactionData {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

impl ToHash256 for ChatLog {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
    }
}

impl ToHash256 for GenesisInfo {
    fn to_hash256(&self) -> Hash256 {
        Hash256::hash(serde_spb::to_vec(self).unwrap())
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
    /// Calculates the `transactions_hash` field.
    ///
    /// Don't confuse with the `impl ToHash256 for Agenda`, which
    /// calculates the hash of the agenda itself.
    pub fn calculate_transactions_hash(transactions: &[Transaction]) -> Hash256 {
        let mut hash = Hash256::zero();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serde_spb;
    use std::mem::size_of;

    unsafe fn read<T: Clone>(offset: &mut usize, data: &[u8]) -> T {
        let size = size_of::<T>();
        let p = data[*offset..*offset + size].as_ptr() as *const T;
        let x = (*p).clone();
        *offset += size;
        x
    }

    #[test]
    fn decode_header() {
        let header = test_utils::generate_standard_genesis(5)
            .0
            .genesis_info
            .header;
        let encoded = serde_spb::to_vec(&header).unwrap();

        let mut offset = 0;

        let author = unsafe { read::<PublicKey>(&mut offset, &encoded) };
        let prev_block_finalization_proof_round =
            unsafe { read::<ConsensusRound>(&mut offset, &encoded) };
        let prev_block_finalization_proof_signatures_len =
            unsafe { read::<usize>(&mut offset, &encoded) };
        let mut prev_block_finalization_proof_signatures = Vec::new();
        for _ in 0..prev_block_finalization_proof_signatures_len {
            prev_block_finalization_proof_signatures.push(unsafe {
                read::<TypedSignature<FinalizationSignTarget>>(&mut offset, &encoded)
            });
        }
        let previous_hash = unsafe { read::<Hash256>(&mut offset, &encoded) };
        let height = unsafe { read::<BlockHeight>(&mut offset, &encoded) };
        let timestamp = unsafe { read::<Timestamp>(&mut offset, &encoded) };
        let commit_merkle_root = unsafe { read::<Hash256>(&mut offset, &encoded) };
        let repository_merkle_root = unsafe { read::<Hash256>(&mut offset, &encoded) };
        let validator_set_len = unsafe { read::<usize>(&mut offset, &encoded) };
        let mut validator_set = Vec::new();
        for _ in 0..validator_set_len {
            let pub_key = unsafe { read::<PublicKey>(&mut offset, &encoded) };
            let voting_power = unsafe { read::<VotingPower>(&mut offset, &encoded) };
            validator_set.push((pub_key, voting_power));
        }
        offset += 8; // skip version length (it's always 5)
        let version = unsafe { read::<[u8; 5]>(&mut offset, &encoded) };
        let version = String::from_utf8(version.to_vec()).unwrap();

        let header_decoded = BlockHeader {
            author,
            prev_block_finalization_proof: FinalizationProof {
                round: prev_block_finalization_proof_round,
                signatures: prev_block_finalization_proof_signatures,
            },
            previous_hash,
            height,
            timestamp,
            commit_merkle_root,
            repository_merkle_root,
            validator_set,
            version,
        };

        // Note that `header` is from `simperby-test-suite`, which is not compatible with
        // this crate. Thus we compare the strings.
        assert_eq!(
            serde_spb::to_string(&header).unwrap(),
            serde_spb::to_string(&header_decoded).unwrap()
        );

        assert_eq!(header, serde_spb::from_slice(&encoded).unwrap());
    }
}
