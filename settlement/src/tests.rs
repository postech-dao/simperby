//! Various test scenarios for the settlement chain communication

use super::*;
use execution::*;
use simperby_common::{verify::CommitSequenceVerifier, *};
use std::time::Duration;
use tokio::time::sleep;

pub struct ChainInfo {
    pub chain_name: String,
    pub last_finalized_header: BlockHeader,
    pub last_finalization_proof: FinalizationProof,
    pub reserved_state: ReservedState,
    /// The private keys of the validators of the next block.
    ///
    /// Both governance and consensus sets must be the same.
    pub validators: Vec<PrivateKey>,
}

impl ChainInfo {
    /// Creates Chain info from the standard genesis test suite.
    ///
    /// This is useful when you want to test the treasury for the first time.
    pub fn standard_genesis(chain_name: String) -> Self {
        let (reserved_state, validators) = test_utils::generate_standard_genesis(4);
        Self {
            chain_name,
            last_finalized_header: reserved_state.genesis_info.header.clone(),
            last_finalization_proof: reserved_state.genesis_info.genesis_proof.clone(),
            reserved_state,
            validators: validators
                .into_iter()
                .map(|(_, private_key)| private_key)
                .collect(),
        }
    }
}

/// A simple transfer scneario.
///
/// The treasury contract must be synchronized with the given `chain_info`
/// and `initial_contract_sequence` as its initial state.
pub async fn scenario_1(
    chain_info: ChainInfo,
    initial_contract_sequence: u128,
    token_address: HexSerializedVec,
    temporary_receiver_address: HexSerializedVec,
    sc: impl SettlementChain,
    transaction_finalization_wait: Duration,
) {
    // Setup the on-chain state
    let mut csv = CommitSequenceVerifier::new(
        chain_info.last_finalized_header.clone(),
        chain_info.reserved_state.clone(),
    )
    .unwrap();

    // Query the initial status
    let initial_balance = sc
        .get_treasury_fungible_token_balance(token_address.clone())
        .await
        .unwrap();
    let initial_treasury_header = sc.get_light_client_header().await.unwrap();
    let contract_sequence = sc.get_contract_sequence().await.unwrap();
    let initial_temporary_receiver_balance = sc
        .eoa_get_fungible_token_balance(temporary_receiver_address.clone(), token_address.clone())
        .await
        .unwrap();
    assert_eq!(initial_treasury_header, chain_info.last_finalized_header);
    assert_eq!(contract_sequence, initial_contract_sequence);

    // Apply transactions
    let mut transactions = Vec::new();
    for i in 0..10 {
        let tx = Transaction {
            author: "doesn't matter".to_owned(),
            timestamp: 0,
            head: format!("commit {i}"),
            body: "".to_owned(),
            diff: Diff::None,
        };
        csv.apply_commit(&Commit::Transaction(tx.clone())).unwrap();
        transactions.push(tx);
    }
    let execute_tx = execution::create_execution_transaction(
        &Execution {
            target_chain: chain_info.chain_name,
            contract_sequence: initial_contract_sequence,
            message: ExecutionMessage::TransferFungibleToken(TransferFungibleToken {
                token_address: token_address.clone(),
                amount: initial_balance,
                receiver_address: temporary_receiver_address.clone(),
            }),
        },
        "hi".to_owned(),
        1234,
    )
    .unwrap();
    csv.apply_commit(&Commit::Transaction(execute_tx.clone()))
        .unwrap();
    transactions.push(execute_tx.clone());
    for i in 0..20 {
        let tx = Transaction {
            author: "doesn't matter".to_owned(),
            timestamp: 0,
            head: format!("commit {i}"),
            body: "".to_owned(),
            diff: Diff::None,
        };
        csv.apply_commit(&Commit::Transaction(tx.clone())).unwrap();
        transactions.push(tx);
    }

    // Complete the block
    let agenda = Agenda {
        height: 1,
        author: chain_info.reserved_state.consensus_leader_order[0].clone(),
        timestamp: 0,
        transactions_hash: Agenda::calculate_transactions_hash(&transactions),
    };
    csv.apply_commit(&Commit::Agenda(agenda.clone())).unwrap();
    csv.apply_commit(&Commit::AgendaProof(AgendaProof {
        height: 1,
        agenda_hash: agenda.to_hash256(),
        proof: chain_info
            .validators
            .iter()
            .map(|private_key| TypedSignature::sign(&agenda, private_key).unwrap())
            .collect::<Vec<_>>(),
        timestamp: 0,
    }))
    .unwrap();
    let block_header = BlockHeader {
        author: chain_info.validators[0].public_key(),
        prev_block_finalization_proof: chain_info.last_finalization_proof,
        previous_hash: chain_info.last_finalized_header.to_hash256(),
        height: 1,
        timestamp: 0,
        commit_merkle_root: BlockHeader::calculate_commit_merkle_root(
            &csv.get_total_commits()[1..],
        ),
        repository_merkle_root: Hash256::zero(),
        validator_set: chain_info.last_finalized_header.validator_set.clone(),
        version: chain_info.last_finalized_header.version,
    };
    csv.apply_commit(&Commit::Block(block_header.clone()))
        .unwrap();
    let signatures = chain_info
        .validators
        .iter()
        .map(|private_key| {
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

    // Update light client
    sc.update_treasury_light_client(block_header.clone(), fp)
        .await
        .unwrap();
    sleep(transaction_finalization_wait).await;
    assert_eq!(sc.get_light_client_header().await.unwrap(), block_header);

    // Execute transfer
    let commits = csv.get_total_commits();
    let merkle_tree = OneshotMerkleTree::create(
        commits[1..=(commits.len() - 2)]
            .iter()
            .map(|c| c.to_hash256())
            .collect(),
    );
    let merkle_proof = merkle_tree
        .create_merkle_proof(execute_tx.to_hash256())
        .unwrap();
    sc.execute(execute_tx, 1, merkle_proof).await.unwrap();
    sleep(transaction_finalization_wait).await;

    // Check the result
    assert_eq!(
        sc.eoa_get_fungible_token_balance(
            temporary_receiver_address.clone(),
            token_address.clone()
        )
        .await
        .unwrap(),
        initial_temporary_receiver_balance + initial_balance
    );
}
