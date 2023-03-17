use light_client::*;
use merkle_tree::*;
use simperby_common::{verify::CommitSequenceVerifier, *};
use simperby_test_suite::*;

#[test]
fn basic1() {
    let member_number = 10;
    let (rs, keys) = test_utils::generate_standard_genesis(member_number);
    let genesis_info = rs.genesis_info.clone();
    let genesis_header = rs.genesis_info.header.clone();

    let mut csv = CommitSequenceVerifier::new(genesis_header.clone(), rs.clone()).unwrap();
    let mut light_client = LightClient::new(genesis_header);

    let tx = Transaction {
        author: "doesn't matter".to_owned(),
        timestamp: 0,
        head: "commit 1".to_owned(),
        body: "".to_owned(),
        diff: Diff::None,
    };
    csv.apply_commit(&Commit::Transaction(tx.clone())).unwrap();
    let agenda = Agenda {
        height: 1,
        author: rs.query_name(&keys[0].0).unwrap(),
        timestamp: 0,
        transactions_hash: Agenda::calculate_transactions_hash(&[tx.clone()]),
    };
    csv.apply_commit(&Commit::Agenda(agenda.clone())).unwrap();
    csv.apply_commit(&Commit::AgendaProof(AgendaProof {
        height: 1,
        agenda_hash: agenda.to_hash256(),
        proof: keys
            .iter()
            .map(|(_, private_key)| TypedSignature::sign(&agenda, private_key).unwrap())
            .collect::<Vec<_>>(),
        timestamp: 0,
    }))
    .unwrap();
    let block_header = BlockHeader {
        author: keys[0].0.clone(),
        prev_block_finalization_proof: genesis_info.genesis_proof,
        previous_hash: genesis_info.header.to_hash256(),
        height: 1,
        timestamp: 0,
        commit_merkle_root: BlockHeader::calculate_commit_merkle_root(
            &csv.get_total_commits()[1..],
        ),
        repository_merkle_root: Hash256::zero(),
        validator_set: genesis_info.header.validator_set.clone(),
        version: genesis_info.header.version,
    };
    csv.apply_commit(&Commit::Block(block_header.clone()))
        .unwrap();
    let signatures = keys
        .iter()
        .map(|(_, private_key)| {
            TypedSignature::sign(
                &FinalizationSignTarget {
                    block_hash: block_header.to_hash256(),
                    round: 0,
                },
                private_key,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let fp = FinalizationProof {
        round: 0,
        signatures,
    };
    csv.verify_last_header_finalization(&fp).unwrap();
    light_client.update(block_header, fp).unwrap();
    let commits = csv.get_total_commits();
    let merkle_tree = OneshotMerkleTree::create(
        commits[1..=(commits.len() - 2)]
            .iter()
            .map(|c| c.to_hash256())
            .collect(),
    );
    let merkle_proof = merkle_tree.create_merkle_proof(tx.to_hash256()).unwrap();
    assert!(light_client.verify_transaction_commitment(&tx, 1, merkle_proof));
}

#[test]
fn basic2() {
    setup_test();
    let member_number = 10;
    let (reserved_state, keys): (ReservedState, Vec<(PublicKey, PrivateKey)>) =
        test_utils::generate_delegated_genesis(member_number, true);
    let genesis_info = reserved_state.genesis_info.clone();
    let genesis_header = reserved_state.genesis_info.header.clone();

    let mut csv =
        CommitSequenceVerifier::new(genesis_header.clone(), reserved_state.clone()).unwrap();
    let mut light_client = LightClient::new(genesis_header);

    let tx = Transaction {
        author: "doesn't matter".to_owned(),
        timestamp: 0,
        head: "commit 1".to_owned(),
        body: "".to_owned(),
        diff: Diff::None,
    };
    csv.apply_commit(&Commit::Transaction(tx.clone())).unwrap();
    let agenda = Agenda {
        height: 1,
        author: reserved_state.query_name(&keys[1].0).unwrap(),
        timestamp: 0,
        transactions_hash: Agenda::calculate_transactions_hash(&[tx.clone()]),
    };
    csv.apply_commit(&Commit::Agenda(agenda.clone())).unwrap();
    csv.apply_commit(&Commit::AgendaProof(AgendaProof {
        height: 1,
        agenda_hash: agenda.to_hash256(),
        proof: keys[1..]
            .iter()
            .map(|(_, private_key)| TypedSignature::sign(&agenda, private_key).unwrap())
            .collect::<Vec<_>>(),
        timestamp: 0,
    }))
    .unwrap();
    let block_header = BlockHeader {
        author: keys[1].0.clone(),
        prev_block_finalization_proof: genesis_info.genesis_proof,
        previous_hash: genesis_info.header.to_hash256(),
        height: 1,
        timestamp: 0,
        commit_merkle_root: BlockHeader::calculate_commit_merkle_root(
            &csv.get_total_commits()[1..],
        ),
        repository_merkle_root: Hash256::zero(),
        validator_set: reserved_state.get_validator_set().unwrap(),
        version: genesis_info.header.version,
    };
    csv.apply_commit(&Commit::Block(block_header.clone()))
        .unwrap();
    let signatures = keys
        .iter()
        .map(|(_, private_key)| {
            TypedSignature::sign(
                &FinalizationSignTarget {
                    block_hash: block_header.to_hash256(),
                    round: 0,
                },
                private_key,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let fp = FinalizationProof {
        round: 0,
        signatures,
    };
    csv.verify_last_header_finalization(&fp).unwrap();
    light_client.update(block_header, fp).unwrap();
    let commits = csv.get_total_commits();
    let merkle_tree = OneshotMerkleTree::create(
        commits[1..=(commits.len() - 2)]
            .iter()
            .map(|c| c.to_hash256())
            .collect(),
    );
    let merkle_proof = merkle_tree.create_merkle_proof(tx.to_hash256()).unwrap();
    assert!(light_client.verify_transaction_commitment(&tx, 1, merkle_proof));
}

#[test]
fn basic3() {
    let member_number = 10;
    let (rs, keys) = test_utils::generate_standard_genesis(member_number);
    let genesis_info = rs.genesis_info.clone();
    let genesis_header = rs.genesis_info.header.clone();

    let (height, timestamp) = (1, 0);
    let mut csv = CommitSequenceVerifier::new(genesis_header.clone(), rs.clone()).unwrap();
    let mut light_client = LightClient::new(genesis_header);

    let tx = Transaction {
        author: "doesn't matter".to_owned(),
        timestamp,
        head: "commit 1".to_owned(),
        body: "".to_owned(),
        diff: Diff::None,
    };
    csv.apply_commit(&Commit::Transaction(tx.clone())).unwrap();

    let agenda = Agenda {
        height,
        author: rs.query_name(&keys[0].0).unwrap(),
        timestamp,
        transactions_hash: Agenda::calculate_transactions_hash(&[tx.clone()]),
    };
    csv.apply_commit(&Commit::Agenda(agenda.clone())).unwrap();

    csv.apply_commit(&Commit::AgendaProof(AgendaProof {
        height,
        agenda_hash: agenda.to_hash256(),
        proof: keys[1..]
            .iter()
            .map(|(_, private_key)| TypedSignature::sign(&agenda, private_key).unwrap())
            .collect::<Vec<_>>(),
        timestamp,
    }))
    .unwrap();

    let data = DelegationTransactionData {
        delegator: rs.members[0].name.to_owned(),
        delegatee: rs.members[2].name.to_owned(),
        governance: true,
        block_height: height,
        timestamp,
        chain_name: "PDAO-mainnet".to_owned(),
    };
    let tx_delegate = ExtraAgendaTransaction::Delegate(TxDelegate {
        proof: TypedSignature::sign(&data, &keys[0].1).unwrap(),
        data,
    });
    csv.apply_commit(&Commit::ExtraAgendaTransaction(tx_delegate))
        .unwrap();

    let block_header = BlockHeader {
        author: keys[0].0.clone(),
        prev_block_finalization_proof: genesis_info.genesis_proof,
        previous_hash: genesis_info.header.to_hash256(),
        height,
        timestamp,
        commit_merkle_root: BlockHeader::calculate_commit_merkle_root(
            &csv.get_total_commits()[1..],
        ),
        repository_merkle_root: Hash256::zero(),
        validator_set: rs.get_validator_set().unwrap(),
        version: genesis_info.header.version.clone(),
    };
    csv.apply_commit(&Commit::Block(block_header.clone()))
        .unwrap();

    let signatures = keys
        .iter()
        .map(|(_, private_key)| {
            TypedSignature::sign(
                &FinalizationSignTarget {
                    block_hash: block_header.to_hash256(),
                    round: 0,
                },
                private_key,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let fp = FinalizationProof {
        round: 0,
        signatures,
    };
    csv.verify_last_header_finalization(&fp).unwrap();
    light_client
        .update(block_header.clone(), fp.clone())
        .unwrap();
    let commits = csv.get_total_commits();
    let merkle_tree = OneshotMerkleTree::create(
        commits[1..=(commits.len() - 2)]
            .iter()
            .map(|c| c.to_hash256())
            .collect(),
    );
    let merkle_proof = merkle_tree.create_merkle_proof(tx.to_hash256()).unwrap();
    assert!(light_client.verify_transaction_commitment(&tx, height, merkle_proof));
    assert_eq!(
        csv.get_reserved_state()
            .get_validator_set()
            .unwrap()
            .iter()
            .find(|(pub_key, _)| pub_key == &keys[0].0),
        None
    );

    let (height, timestamp) = (2, 1);

    assert_eq!(
        rs.get_governance_set()
            .unwrap()
            .iter()
            .find(|(pub_key, _)| { pub_key == &keys[1].0 }),
        Some(&(keys[1].0.clone(), 1))
    );
    let agenda = Agenda {
        height,
        author: rs.query_name(&keys[1].0).unwrap(),
        timestamp,
        transactions_hash: Agenda::calculate_transactions_hash(&[]),
    };
    csv.apply_commit(&Commit::Agenda(agenda.clone())).unwrap();

    assert!(matches!(
        csv.apply_commit(&Commit::AgendaProof(AgendaProof {
            height,
            agenda_hash: agenda.to_hash256(),
            proof: keys
                .iter()
                .map(|(_, private_key)| TypedSignature::sign(&agenda, private_key).unwrap())
                .collect::<Vec<_>>(),
            timestamp
        })),
        Err(verify::Error::InvalidArgument(_)),
    ));

    csv.apply_commit(&Commit::AgendaProof(AgendaProof {
        height,
        agenda_hash: agenda.to_hash256(),
        proof: keys[1..]
            .iter()
            .map(|(_, private_key)| TypedSignature::sign(&agenda, private_key).unwrap())
            .collect::<Vec<_>>(),
        timestamp,
    }))
    .unwrap();

    let data = UndelegationTransactionData {
        delegator: rs.members[0].name.to_owned(),
        block_height: height,
        timestamp,
        chain_name: "PDAO-mainnet".to_owned(),
    };
    let tx_delegate = ExtraAgendaTransaction::Undelegate(TxUndelegate {
        proof: TypedSignature::sign(&data, &keys[0].1).unwrap(),
        data,
    });
    csv.apply_commit(&Commit::ExtraAgendaTransaction(tx_delegate))
        .unwrap();

    let block_header = BlockHeader {
        author: keys[1].0.clone(),
        prev_block_finalization_proof: fp,
        previous_hash: block_header.to_hash256(),
        height,
        timestamp,
        commit_merkle_root: BlockHeader::calculate_commit_merkle_root(
            &csv.get_total_commits()[5..],
        ),
        repository_merkle_root: Hash256::zero(),
        validator_set: rs.get_validator_set().unwrap(),
        version: genesis_info.header.version,
    };
    csv.apply_commit(&Commit::Block(block_header.clone()))
        .unwrap();
    let signatures = keys
        .iter()
        .map(|(_, private_key)| {
            TypedSignature::sign(
                &FinalizationSignTarget {
                    block_hash: block_header.to_hash256(),
                    round: 0,
                },
                private_key,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let fp = FinalizationProof {
        round: 0,
        signatures,
    };
    csv.verify_last_header_finalization(&fp).unwrap();
    light_client.update(block_header, fp).unwrap();

    assert_eq!(
        csv.get_reserved_state()
            .get_validator_set()
            .unwrap()
            .iter()
            .find(|(pub_key, _)| pub_key == &keys[0].0),
        Some(&(keys[0].0.clone(), 1))
    );
}
