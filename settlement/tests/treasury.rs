//! An example of a settlement chain (Mythereum) abstracted in plain Rust code.
//!
//! When implementing a settlement treasury, this example will be helpful to understand the basic structure of message delivery.

use light_client::LightClient;
use merkle_tree::*;
use rust_decimal::Decimal;
use simperby_common::merkle_tree::MerkleProof;
use simperby_common::verify::CommitSequenceVerifier;
use simperby_common::*;
use simperby_settlement::execution::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

fn string_to_hex(s: &str) -> HexSerializedVec {
    HexSerializedVec::from(s.as_bytes().to_vec())
}

pub struct TetherContract {
    balances: HashMap<HexSerializedVec, Decimal>,
}

/// The interface for a fungible token, like the ERC20 standard.
pub trait MRC20 {
    fn get_balance(&self, address: &HexSerializedVec) -> Decimal;
    fn transfer(
        &mut self,
        context: &mut GlobalContext,
        to: &HexSerializedVec,
        amount: Decimal,
    ) -> bool;
}

impl MRC20 for TetherContract {
    fn get_balance(&self, address: &HexSerializedVec) -> Decimal {
        *self.balances.get(address).unwrap_or(&Decimal::ZERO)
    }

    fn transfer(
        &mut self,
        context: &mut GlobalContext,
        to: &HexSerializedVec,
        amount: Decimal,
    ) -> bool {
        let from_balance = self.get_balance(&context.caller);
        if from_balance < amount {
            return false;
        }
        let to_balance = self.get_balance(to);
        self.balances
            .insert(context.caller.clone(), from_balance - amount);
        self.balances.insert(to.clone(), to_balance + amount);
        true
    }
}

pub struct GlobalContext {
    pub tether: Rc<RefCell<TetherContract>>,
    pub caller: HexSerializedVec,
}

pub struct MythereumTreasuryContract {
    light_client: light_client::LightClient,
    sequence: u128,
}

impl MythereumTreasuryContract {
    pub fn new(header: BlockHeader) -> Result<Self, String> {
        let light_client = light_client::LightClient::new(header);
        Ok(Self {
            light_client,
            sequence: 0,
        })
    }

    /// A transaction handler for the light client update.
    pub fn update_light_client(
        &mut self,
        _context: &mut GlobalContext,
        header: BlockHeader,
        proof: FinalizationProof,
    ) -> Result<(), String> {
        self.light_client.update(header, proof)
    }

    /// A transaction handler for the execution.
    pub fn execute(
        &mut self,
        context: &mut GlobalContext,
        execution_transaction: Transaction,
        simperby_height: BlockHeight,
        proof: MerkleProof,
    ) -> Result<(), String> {
        let execution = convert_transaction_to_execution(&execution_transaction)?;
        if execution.contract_sequence != self.sequence {
            return Err("Invalid sequence".to_string());
        }
        if execution.target_chain != "mythereum" {
            return Err("Invalid target chain".to_string());
        }

        if !self.light_client.verify_transaction_commitment(
            &execution_transaction,
            simperby_height,
            proof,
        ) {
            return Err("Invalid proof".to_string());
        }

        match execution.message {
            ExecutionMessage::Dummy { msg } => {
                unimplemented!("Should emit an event with the message ({})", msg)
            }
            ExecutionMessage::TransferFungibleToken(TransferFungibleToken {
                token_address,
                amount,
                receiver_address,
            }) => {
                if token_address != string_to_hex("tether-address") {
                    unimplemented!()
                }
                let tether_rc = Rc::clone(&context.tether);
                let mut tether = tether_rc.borrow_mut();
                context.caller = string_to_hex("treasury-address");
                if !tether.transfer(context, &receiver_address, amount) {
                    return Err("Insufficient balance".to_string());
                }
            }
            ExecutionMessage::TransferNonFungibleToken(_) => todo!(),
        }

        self.sequence += 1;
        Ok(())
    }
}

#[test]
fn relay_1() {
    let (reserved_state, keys) = test_utils::generate_standard_genesis(4);
    let genesis_info = reserved_state.genesis_info.clone();
    let genesis_header = reserved_state.genesis_info.header.clone();

    let mut csv = CommitSequenceVerifier::new(
        reserved_state.genesis_info.header.clone(),
        reserved_state.clone(),
    )
    .unwrap();
    let tx1 = create_execution_transaction(
        &Execution {
            target_chain: "mythereum".to_string(),
            contract_sequence: 0,
            message: ExecutionMessage::TransferFungibleToken(TransferFungibleToken {
                token_address: string_to_hex("tether-address"),
                amount: Decimal::new(100, 0),
                receiver_address: string_to_hex("receiver-address"),
            }),
        },
        "doesn't matter".to_owned(),
        0,
    )
    .unwrap();
    let tx2 = create_execution_transaction(
        &Execution {
            target_chain: "mythereum".to_string(),
            contract_sequence: 1,
            message: ExecutionMessage::TransferFungibleToken(TransferFungibleToken {
                token_address: string_to_hex("tether-address"),
                amount: Decimal::new(200, 0),
                receiver_address: string_to_hex("receiver-address"),
            }),
        },
        "doesn't matter".to_owned(),
        0,
    )
    .unwrap();
    csv.apply_commit(&Commit::Transaction(tx1.clone())).unwrap();
    csv.apply_commit(&Commit::Transaction(tx2.clone())).unwrap();

    let agenda = Agenda {
        height: 1,
        author: reserved_state.query_name(&keys[0].0).unwrap(),
        timestamp: 0,
        transactions_hash: Agenda::calculate_transactions_hash(&[tx1.clone(), tx2.clone()]),
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
        author: keys[0].0.clone(), // Note that keys[0] is member-0001
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
    let fp = keys
        .iter()
        .map(|(_, private_key)| TypedSignature::sign(&block_header, private_key).unwrap())
        .collect::<Vec<_>>();

    // Setup Mythereum
    let tether = Rc::new(RefCell::new(TetherContract {
        balances: vec![(string_to_hex("treasury-address"), Decimal::new(299, 0))]
            .into_iter()
            .collect(),
    }));
    let mut context = GlobalContext {
        caller: string_to_hex("some-eoa"),
        tether,
    };
    let mut treasury = MythereumTreasuryContract {
        light_client: LightClient::new(genesis_header),
        sequence: 0,
    };
    treasury
        .update_light_client(&mut context, block_header.clone(), fp)
        .unwrap();
    let merkle_tree = OneshotMerkleTree::create(
        csv.get_total_commits()[1..=4]
            .iter()
            .map(|c| c.to_hash256())
            .collect(),
    );
    assert_eq!(merkle_tree.root(), block_header.commit_merkle_root);
    let merkle_proof = merkle_tree.create_merkle_proof(tx1.to_hash256()).unwrap();
    treasury
        .execute(&mut context, tx1, 1, merkle_proof)
        .unwrap();
    let merkle_proof = merkle_tree.create_merkle_proof(tx2.to_hash256()).unwrap();
    treasury
        .execute(&mut context, tx2, 1, merkle_proof)
        .unwrap_err();
}
