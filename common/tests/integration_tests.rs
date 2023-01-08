use light_client::*;
use merkle_tree::*;
use simperby_common::{verify::CommitSequenceVerifier, *};
use simperby_test_suite::*;

#[test]
fn basic1() {
    let member_number = 10;
    let (rs, keys) = generate_standard_genesis(member_number);
    let genesis_info = rs.genesis_info.clone();
    let genesis_header = rs.genesis_info.header.clone();

    let mut csv = CommitSequenceVerifier::new(genesis_header.clone(), rs).unwrap();
    let mut light_client = LightClient::new(genesis_header);

    let tx = Transaction {
        author: PublicKey::zero(),
        timestamp: 0,
        head: "commit 1".to_owned(),
        body: "".to_owned(),
        diff: Diff::None,
    };
    csv.apply_commit(&Commit::Transaction(tx.clone())).unwrap();
    let agenda = Agenda {
        height: 1,
        author: keys[0].0.clone(),
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
    let fp = keys
        .iter()
        .map(|(_, private_key)| TypedSignature::sign(&block_header, private_key).unwrap())
        .collect::<Vec<_>>();
    csv.verify_last_header_finalization(&fp).unwrap();
    light_client.update(block_header, fp).unwrap();
    let merkle_tree = OneshotMerkleTree::create(
        csv.get_total_commits()[1..=3]
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
        generate_delegated_genesis(member_number);
    let genesis_info = reserved_state.genesis_info.clone();
    let genesis_header = reserved_state.genesis_info.header.clone();

    let mut csv =
        CommitSequenceVerifier::new(genesis_header.clone(), reserved_state.clone()).unwrap();
    let mut light_client = LightClient::new(genesis_header);

    let tx = Transaction {
        author: PublicKey::zero(),
        timestamp: 0,
        head: "commit 1".to_owned(),
        body: "".to_owned(),
        diff: Diff::None,
    };
    csv.apply_commit(&Commit::Transaction(tx.clone())).unwrap();
    let agenda = Agenda {
        height: 1,
        author: keys[0].0.clone(), // Note that keys[0] is member-0001
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
    }))
    .unwrap();
    let block_header = BlockHeader {
        author: keys[0].0.clone(), // Note that keys[0] is member-0001
        prev_block_finalization_proof: genesis_info.genesis_proof,
        previous_hash: genesis_info.header.to_hash256(),
        height: 1,
        timestamp: 0,
        commit_merkle_root: BlockHeader::calculate_commit_merkle_root(
            &csv.get_total_commits()[1..],
        ),
        repository_merkle_root: Hash256::zero(),
        // Note that validator set here is from member-0001 to member-0009
        validator_set: reserved_state.get_validator_set().unwrap(),
        version: genesis_info.header.version,
    };
    csv.apply_commit(&Commit::Block(block_header.clone()))
        .unwrap();
    let fp = keys
        .iter()
        .map(|(_, private_key)| TypedSignature::sign(&block_header, private_key).unwrap())
        .collect::<Vec<_>>();
    csv.verify_last_header_finalization(&fp).unwrap();
    light_client.update(block_header, fp).unwrap();
    let merkle_tree = OneshotMerkleTree::create(
        csv.get_total_commits()[1..=3]
            .iter()
            .map(|c| c.to_hash256())
            .collect(),
    );
    let merkle_proof = merkle_tree.create_merkle_proof(tx.to_hash256()).unwrap();
    assert!(light_client.verify_transaction_commitment(&tx, 1, merkle_proof));
}
