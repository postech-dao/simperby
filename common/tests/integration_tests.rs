use light_client::*;
use merkle_tree::*;
use simperby_common::{verify::CommitSequenceVerifier, *};
use simperby_test_suite::*;
use std::collections::HashSet;

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
    assert!(light_client.verify_commitment(serde_json::to_vec(&tx).unwrap(), 1, merkle_proof));
}

#[test]
fn basic2() {
    setup_test();
    let member_number = 10;
    let del_genesis = generate_delegation_genesis(member_number);
    let mut rs: ReservedState = del_genesis.0;
    let _keys: Vec<(PublicKey, PrivateKey)> = del_genesis.1;

    // Check get_validator_set
    let mut val_set_initial = rs.clone().genesis_info.header.validator_set;
    let index = val_set_initial
        .iter()
        .enumerate()
        .position(|(x, _)| x == 0)
        .unwrap();
    val_set_initial[2].1 = 2;
    val_set_initial.remove(index);

    rs.consensus_leader_order.remove(0);
    let get_val_set = rs.get_validator_set().unwrap();
    assert_eq!(val_set_initial, get_val_set);

    // Check get_governance_set
    let get_gov_set = rs
        .get_governance_set()
        .unwrap()
        .into_iter()
        .collect::<HashSet<_>>();
    let mut member_governance: Vec<(PublicKey, VotingPower)> = rs
        .members
        .iter()
        .map(|m| (m.clone().public_key, m.governance_voting_power))
        .collect();
    let index = val_set_initial
        .iter()
        .enumerate()
        .position(|(x, _)| x == 0)
        .unwrap();
    member_governance[2].1 = 2;
    member_governance.remove(index);
    let member_governance = member_governance.into_iter().collect::<HashSet<_>>();
    rs.members.remove(0);

    assert_eq!(member_governance, get_gov_set);
}
