use light_client::*;
use merkle_tree::*;
use reserved::ReservedState;
use simperby_common::{verify::CommitSequenceVerifier, *};

#[test]
fn basic1() {
    let n = 10;
    let keys = (0..n)
        .into_iter()
        .map(|i| generate_keypair(format!("{}", i)))
        .collect::<Vec<_>>();
    let members = keys
        .iter()
        .enumerate()
        .map(|(i, (public_key, _))| Member {
            public_key: public_key.clone(),
            name: format!("member-{}", i),
            governance_voting_power: 1,
            consensus_voting_power: 1,
            governance_delegations: None,
            consensus_delegations: None,
        })
        .collect::<Vec<_>>();
    let genesis_header = BlockHeader {
        author: PublicKey::zero(),
        prev_block_finalization_proof: Vec::new(),
        previous_hash: Hash256::zero(),
        height: 0,
        timestamp: 0,
        commit_merkle_root: Hash256::zero(),
        repository_merkle_root: Hash256::zero(),
        validator_set: members
            .iter()
            .map(|member| (member.public_key.clone(), member.consensus_voting_power))
            .collect::<Vec<_>>(),
        version: "0.1.0".to_string(),
    };
    let genesis_info = GenesisInfo {
        header: genesis_header.clone(),
        genesis_proof: keys
            .iter()
            .map(|(_, secret_key)| TypedSignature::sign(&genesis_header, secret_key).unwrap())
            .collect::<Vec<_>>(),
        chain_name: "test-chain".to_string(),
    };
    let rs = ReservedState {
        genesis_info: genesis_info.clone(),
        members,
        consensus_leader_order: (0..n).into_iter().collect::<Vec<_>>(),
        version: "0.1.0".to_string(),
    };

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
        hash: Agenda::calculate_hash(&[tx.clone()]),
    };
    csv.apply_commit(&Commit::Agenda(agenda.clone())).unwrap();
    csv.apply_commit(&Commit::AgendaProof(AgendaProof {
        height: 1,
        agenda_hash: agenda.to_hash256(),
        proof: keys
            .iter()
            .map(|(_, secret_key)| TypedSignature::sign(&agenda, secret_key).unwrap())
            .collect::<Vec<_>>(),
    }))
    .unwrap();
    let block_header = BlockHeader {
        author: keys[0].0.clone(),
        prev_block_finalization_proof: genesis_info.genesis_proof,
        previous_hash: genesis_info.header.to_hash256(),
        height: 1,
        timestamp: 0,
        commit_merkle_root: BlockHeader::calculate_commit_merkle_root(&csv.get_commits()[1..]),
        repository_merkle_root: Hash256::zero(),
        validator_set: genesis_info.header.validator_set.clone(),
        version: genesis_info.header.version,
    };
    csv.apply_commit(&Commit::Block(block_header.clone()))
        .unwrap();
    let fp = keys
        .iter()
        .map(|(_, secret_key)| TypedSignature::sign(&block_header, secret_key).unwrap())
        .collect::<Vec<_>>();
    csv.verify_last_header_finalization(&fp).unwrap();
    light_client.update(block_header, fp).unwrap();
    let merkle_tree = OneshotMerkleTree::create(
        csv.get_commits()[1..=3]
            .iter()
            .map(|c| c.to_hash256())
            .collect(),
    );
    let merkle_proof = merkle_tree.create_merkle_proof(tx.to_hash256()).unwrap();
    assert!(light_client.verify_commitment(serde_json::to_vec(&tx).unwrap(), 1, merkle_proof));
}
