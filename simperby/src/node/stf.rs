//! The definition of the state transition function which deduces
//! the next block from the current state, current block and the next given block.

use super::state_storage::*;
use crate::*;

#[derive(Error, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    /// The first failed transaction (later transactions will not be executed).
    #[error("transaction failed: index({index:}) / {message:}")]
    TransactionFailure { index: usize, message: String },
    #[error("state storage error: {0}")]
    StateStorageError(StateStorageError),
    #[error("given header is invalid: {0}")]
    InvalidHeader(String),
    #[error("given state is invalid: {0}")]
    InvalidState(String),
}

impl From<StateStorageError> for ExecutionError {
    fn from(e: StateStorageError) -> Self {
        ExecutionError::StateStorageError(e)
    }
}

/// Partial data about the `BlockHeader` that is not part of the block execution result.
/// In other words, the set of information that is needed to fill the next `BlockHeader`
/// but can't be deduced from the next block itself.
pub struct HeaderInfo {
    pub author: PublicKey,
    pub prev_block_finalization_proof: FinalizationProof,
    pub timestamp: Timestamp,
}

/// Executes the block, updating the storage.
///
/// Note that it does not interact with the storage checkpoint;
/// the caller must check the result and
/// revert the state if it is not successful.
pub(super) async fn execute_block(
    storage: &mut StateStorage,
    last_header: BlockHeader,
    transactions: Vec<Transaction>,
    header_info: HeaderInfo,
) -> Result<BlockHeader, ExecutionError> {
    let mut next_version = last_header.version.clone();
    for (index, transaction) in transactions.iter().enumerate() {
        if let Some(state_transition) = &transaction.state_transition {
            match state_transition {
                StateTransition::AddValidator {
                    public_key,
                    voting_power,
                } => {
                    execute_add_validator(storage, index, public_key.clone(), *voting_power)
                        .await?;
                }
                StateTransition::RemoveValidator { public_key } => {
                    execute_remove_validator(storage, index, public_key.clone()).await?;
                }
                StateTransition::Delegate {
                    delegator,
                    delegatee,
                    target_height,
                    commitment_proof,
                } => {
                    execute_delegate(
                        storage,
                        &last_header,
                        index,
                        delegator.clone(),
                        delegatee.clone(),
                        *target_height,
                        commitment_proof.clone(),
                    )
                    .await?;
                }
                StateTransition::Undelegate { delegator } => {
                    execute_undelegate(storage, index, delegator.clone()).await?;
                }
                StateTransition::InsertOrUpdateData { key, value } => {
                    storage
                        .insert_or_update_data(Hash256::hash(key), value)
                        .await?;
                }
                StateTransition::RemoveData(key) => {
                    storage.remove_data(Hash256::hash(key)).await?;
                }
                StateTransition::UpdateVersion { version } => {
                    let ver1 = semver::Version::parse(&last_header.version).map_err(|_| {
                        ExecutionError::InvalidHeader(format!(
                            "invalid version: {}",
                            last_header.version
                        ))
                    })?;
                    let ver2 = semver::Version::parse(version).map_err(|_| {
                        ExecutionError::InvalidHeader(format!("invalid version: {}", version))
                    })?;
                    if ver2 <= ver1 {
                        return Err(ExecutionError::InvalidHeader(format!(
                            "version must grow: got {} but currently {}",
                            ver2, ver1
                        )));
                    }
                    next_version = ver2.to_string();
                }
            }
        }
    }
    let merkle_tree = simperby_merkle_tree::oneshot::OneshotMerkleTree::create(
        transactions.iter().map(|t| t.hash()).collect(),
    );
    let delegation_state = storage.get_delegation_state().await?;
    Ok(BlockHeader {
        author: header_info.author,
        prev_block_finalization_proof: header_info.prev_block_finalization_proof,
        previous_hash: last_header.hash(),
        height: last_header.height + 1,
        timestamp: header_info.timestamp,
        tx_merkle_root: merkle_tree.root(),
        state_merkle_root: storage.state_root().await?,
        validator_set: delegation_state
            .calculate_effective_validator_set()
            .map_err(ExecutionError::InvalidState)?,
        delegation_state_hash: delegation_state.hash(),
        version: next_version,
    })
}

async fn execute_add_validator(
    storage: &mut StateStorage,
    index: usize,
    public_key: PublicKey,
    voting_power: VotingPower,
) -> Result<(), ExecutionError> {
    let mut delegation_state = storage.get_delegation_state().await?;
    if delegation_state
        .original_validator_set
        .iter()
        .map(|(x, _, _)| x)
        .any(|x| *x == public_key)
    {
        return Err(ExecutionError::TransactionFailure {
            index,
            message: "validator already exists".to_string(),
        });
    }
    delegation_state
        .original_validator_set
        .push((public_key.clone(), voting_power, None));
    storage.update_delegation_state(delegation_state).await?;
    Ok(())
}

async fn execute_remove_validator(
    storage: &mut StateStorage,
    index: usize,
    public_key: PublicKey,
) -> Result<(), ExecutionError> {
    let mut delegation_state = storage.get_delegation_state().await?;
    let validator_index = delegation_state
        .original_validator_set
        .iter()
        .position(|(x, _, _)| *x == public_key)
        .ok_or_else(|| ExecutionError::TransactionFailure {
            index,
            message: "validator does not exist".to_string(),
        })?;
    for validator in delegation_state.original_validator_set.iter_mut() {
        if let Some((_, current_delegatee)) = validator.2 {
            if current_delegatee == validator_index {
                validator.2 = None;
            }
        }
    }
    delegation_state
        .original_validator_set
        .remove(validator_index);

    storage.update_delegation_state(delegation_state).await?;
    Ok(())
}

async fn execute_delegate(
    storage: &mut StateStorage,
    last_header: &BlockHeader,
    index: usize,
    delegator: PublicKey,
    delegatee: PublicKey,
    target_height: BlockHeight,
    commitment: TypedSignature<(PublicKey, BlockHeight)>,
) -> Result<(), ExecutionError> {
    if target_height != last_header.height + 1 {
        return Err(ExecutionError::TransactionFailure {
            index,
            message: format!(
                "target height does not match: expected {}, got {}",
                last_header.height + 1,
                target_height
            ),
        });
    }
    if delegatee == delegator {
        return Err(ExecutionError::TransactionFailure {
            index,
            message: "delegator and delegatee are the same".to_string(),
        });
    }
    commitment
        .verify(&(delegatee.clone(), target_height), &delegator)
        .map_err(|e| ExecutionError::TransactionFailure {
            index,
            message: format!("delegation signature verification failed: {}", e),
        })?;

    let mut delegation_state = storage.get_delegation_state().await?;
    let delegator_index = delegation_state
        .original_validator_set
        .iter()
        .position(|(x, _, _)| *x == delegator)
        .ok_or_else(|| ExecutionError::TransactionFailure {
            index,
            message: "delegator does not exist".to_string(),
        })?;
    let delegatee_index = delegation_state
        .original_validator_set
        .iter()
        .position(|(x, _, _)| *x == delegatee)
        .ok_or_else(|| ExecutionError::TransactionFailure {
            index,
            message: "delegatee does not exist".to_string(),
        })?;

    // update current_delegatee for all its delegators, if they exist. (i.e., re-delegation)
    for validator in delegation_state.original_validator_set.iter_mut() {
        if let Some((_, current_delegatee)) = validator.2.as_mut() {
            if *current_delegatee == delegator_index {
                *current_delegatee = delegatee_index;
            }
        }
    }

    if let Some(x) = delegation_state.original_validator_set[delegator_index].2 {
        return Err(ExecutionError::TransactionFailure {
            index,
            message: format!(
                "delegator already has delegated ({:?}); please undelegate first",
                x
            ),
        });
    } else {
        delegation_state.original_validator_set[delegator_index].2 =
            Some((delegatee_index, delegatee_index));
    }

    storage.update_delegation_state(delegation_state).await?;
    Ok(())
}

async fn execute_undelegate(
    storage: &mut StateStorage,
    index: usize,
    delegator: PublicKey,
) -> Result<(), ExecutionError> {
    let mut delegation_state = storage.get_delegation_state().await?;
    let delegator_index = delegation_state
        .original_validator_set
        .iter()
        .position(|(x, _, _)| *x == delegator)
        .ok_or_else(|| ExecutionError::TransactionFailure {
            index,
            message: "delegator does not exist".to_string(),
        })?;
    if delegation_state.original_validator_set[delegator_index]
        .2
        .is_some()
    {
        delegation_state.original_validator_set[delegator_index].2 = None;
    } else {
        return Err(ExecutionError::TransactionFailure {
            index,
            message: "delegator has not delegated".to_string(),
        });
    }

    storage.update_delegation_state(delegation_state).await?;
    Ok(())
}
