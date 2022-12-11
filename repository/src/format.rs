use crate::raw::SemanticCommit;
use eyre::{eyre, Error};
use regex::Regex;
use simperby_common::*;

pub fn to_semantic_commit(commit: &Commit) -> SemanticCommit {
    match commit {
        Commit::Agenda(agenda) => {
            let title = format!(">agenda: {}", agenda.height);
            let body = serde_json::to_string(agenda).unwrap();
            SemanticCommit {
                title,
                body,
                diff: Diff::None,
            }
        }
        Commit::Block(block_header) => {
            let title = format!(">block: {}", block_header.height);
            let body = serde_json::to_string(block_header).unwrap();
            SemanticCommit {
                title,
                body,
                diff: Diff::None,
            }
        }
        Commit::Transaction(transaction) => SemanticCommit {
            title: transaction.head.clone(),
            body: transaction.body.clone(),
            diff: transaction.diff.clone(),
        },
        Commit::AgendaProof(agenda_proof) => {
            let title = format!(">agenda-proof: {}", agenda_proof.height);
            let body = serde_json::to_string(agenda_proof).unwrap();
            SemanticCommit {
                title,
                body,
                diff: Diff::None,
            }
        }
        Commit::ExtraAgendaTransaction(_) => unimplemented!(),
        Commit::ChatLog(_) => unimplemented!(),
    }
}

/// Converts a semantic commit to a commit.
///
/// TODO: retrieve author and timestamp from the commit metadata.
pub fn from_semantic_commit(semantic_commit: SemanticCommit) -> Result<Commit, Error> {
    let pattern = Regex::new(r"^>((agenda)|(block)|(agenda-proof)): (\d+)$").unwrap();
    let captures = pattern.captures(&semantic_commit.title);
    if let Some(captures) = captures {
        let commit_type = captures.get(1).map(|m| m.as_str()).ok_or_else(|| {
            eyre!(
                "Failed to parse commit type from commit title: {}",
                semantic_commit.title
            )
        })?;
        let height = captures.get(5).map(|m| m.as_str()).ok_or_else(|| {
            eyre!(
                "Failed to parse commit height from commit title: {}",
                semantic_commit.title
            )
        })?;
        let height = height.parse::<u64>()?;
        match commit_type {
            "agenda" => {
                let agenda: Agenda = serde_json::from_str(&semantic_commit.body)?;
                if height != agenda.height {
                    return Err(eyre!(
                        "agenda height mismatch: expected {}, got {}",
                        agenda.height,
                        height
                    ));
                }
                Ok(Commit::Agenda(agenda))
            }
            "block" => {
                let block_header: BlockHeader = serde_json::from_str(&semantic_commit.body)?;
                if height != block_header.height {
                    return Err(eyre!(
                        "block height mismatch: expected {}, got {}",
                        block_header.height,
                        height
                    ));
                }
                Ok(Commit::Block(block_header))
            }
            "agenda-proof" => {
                let agenda_proof: AgendaProof = serde_json::from_str(&semantic_commit.body)?;
                if height != agenda_proof.height {
                    return Err(eyre!(
                        "agenda-proof height mismatch: expected {}, got {}",
                        agenda_proof.height,
                        height
                    ));
                }
                Ok(Commit::AgendaProof(agenda_proof))
            }
            _ => Err(eyre!("unknown commit type: {}", commit_type)),
        }
    } else {
        Ok(Commit::Transaction(Transaction {
            author: PublicKey::zero(),
            timestamp: 0,
            head: semantic_commit.title,
            body: semantic_commit.body,
            diff: semantic_commit.diff,
        }))
    }
}

pub fn fp_to_semantic_commit(fp: &LastFinalizationProof) -> SemanticCommit {
    let title = format!(">fp: {}", fp.height);
    let body = serde_json::to_string(&fp).unwrap();
    SemanticCommit {
        title,
        body,
        diff: Diff::None,
    }
}

pub fn fp_from_semantic_commit(
    semantic_commit: SemanticCommit,
) -> Result<LastFinalizationProof, Error> {
    let pattern = Regex::new(r"^>fp: (\d+)$").unwrap();
    let captures = pattern.captures(&semantic_commit.title);
    if let Some(captures) = captures {
        let height = captures.get(1).map(|m| m.as_str()).ok_or_else(|| {
            eyre!(
                "Failed to parse commit height from commit title: {}",
                semantic_commit.title
            )
        })?;
        let height = height.parse::<u64>()?;
        let proof: LastFinalizationProof = serde_json::from_str(&semantic_commit.body)?;
        if height != proof.height {
            return Err(eyre!(
                "proof height mismatch: expected {}, got {}",
                proof.height,
                height
            ));
        }
        Ok(proof)
    } else {
        Err(eyre!("unknown commit type: {}", semantic_commit.title))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_transaction_commit() {
        let transaction = Commit::Transaction(Transaction {
            author: PublicKey::zero(),
            timestamp: 0,
            head: "abc".to_string(),
            body: "def".to_string(),
            diff: Diff::None,
        });
        assert_eq!(
            transaction,
            from_semantic_commit(to_semantic_commit(&transaction)).unwrap()
        );
    }

    #[test]
    fn format_agenda_commit() {
        let agenda = Commit::Agenda(Agenda {
            height: 3,
            author: PublicKey::zero(),
            timestamp: 123,
            transactions_hash: Hash256::hash("hello"),
        });
        assert_eq!(
            agenda,
            from_semantic_commit(to_semantic_commit(&agenda)).unwrap()
        );
    }

    #[test]
    fn format_block_commit() {
        let block = Commit::Block(BlockHeader {
            height: 3,
            author: PublicKey::zero(),
            prev_block_finalization_proof: vec![TypedSignature::new(
                Signature::zero(),
                PublicKey::zero(),
            )],
            previous_hash: Hash256::hash("hello1"),
            timestamp: 0,
            commit_merkle_root: Hash256::hash("hello2"),
            repository_merkle_root: Hash256::hash("hello3"),
            validator_set: vec![(PublicKey::zero(), 1)],
            version: SIMPERBY_CORE_PROTOCOL_VERSION.to_string(),
        });
        assert_eq!(
            block,
            from_semantic_commit(to_semantic_commit(&block)).unwrap()
        );
    }

    #[test]
    fn format_agenda_proof_commit() {
        let agenda_proof = Commit::AgendaProof(AgendaProof {
            height: 3,
            agenda_hash: Hash256::hash("hello1"),
            proof: vec![TypedSignature::new(Signature::zero(), PublicKey::zero())],
        });
        assert_eq!(
            agenda_proof,
            from_semantic_commit(to_semantic_commit(&agenda_proof)).unwrap()
        );
    }

    #[test]
    fn format_fp() {
        let fp = LastFinalizationProof {
            height: 3,
            proof: vec![
                TypedSignature::new(Signature::zero(), PublicKey::zero()),
                TypedSignature::new(Signature::zero(), PublicKey::zero()),
            ],
        };
        assert_eq!(
            fp,
            fp_from_semantic_commit(fp_to_semantic_commit(&fp)).unwrap()
        );
    }
}
