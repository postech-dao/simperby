pub mod execution;
pub mod tests;

use eyre::Error;
use merkle_tree::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use simperby_common::*;

/// An abstract information about a block from a settlement chain.
#[derive(PartialEq, Eq, Debug, Serialize, Deserialize, Clone)]
pub struct SettlementChainBlock {
    /// The height of the block.
    pub height: u64,
    /// The UNIX timestamp of the block in seconds.
    pub timestamp: u64,
}

/// An abstraction of a settlement chain with its treasury deployed on it.
///
/// One trivial implementation of this trait would carry the API endpoint of the full node and
/// the relayer account used to submit message delivering transactions.
#[async_trait::async_trait]
pub trait SettlementChain: Send + Sync {
    /// Returns the name of the chain.
    async fn get_chain_name(&self) -> String;

    /// Checks whether the chain is healthy and the full node is running.
    async fn check_connection(&self) -> Result<(), Error>;

    /// Gets the latest finalized block on the chain.
    async fn get_last_block(&self) -> Result<SettlementChainBlock, Error>;

    /// Returns the current sequence of the treasury contract.
    async fn get_contract_sequence(&self) -> Result<u128, Error>;

    /// Returns the address and the current balance (which is used to pay the gas fee) of the relayer account in this chain.
    ///
    /// The relayer account has no special authority; it is simply used to pay the gas fee for the transaction.
    /// (i.e., there is no need for the contract to check the transaction submitter).
    async fn get_relayer_account_info(&self) -> Result<(HexSerializedVec, Decimal), Error>;

    /// Returns the latest header that the light client has verified.
    async fn get_light_client_header(&self) -> Result<BlockHeader, Error>;

    /// Returns the current balance of a particular fungible token in the treasury contract.
    async fn get_treasury_fungible_token_balance(
        &self,
        address: HexSerializedVec,
    ) -> Result<Decimal, Error>;

    /// Returns the current balance of a particular non-fungible token collection in the treasury contract,
    /// identified as their token indices.
    async fn get_treasury_non_fungible_token_balance(
        &self,
        address: HexSerializedVec,
    ) -> Result<Vec<HexSerializedVec>, Error>;

    /// Updates the light client state in the treasury by providing the next, valid block header and its proof.
    ///
    /// This is one of the message delivery methods; a transaction that carries the given data will be submitted to the chain.
    async fn update_treasury_light_client(
        &self,
        header: BlockHeader,
        proof: FinalizationProof,
    ) -> Result<(), Error>;

    /// Delivers an execution transaction to the settlement chain with the commitment proof.
    ///
    /// - `transaction`: The transaction to deliver.
    /// - `block_height`: The height of the block that the transaction is included in.
    async fn execute(
        &self,
        transaction: Transaction,
        block_height: u64,
        proof: MerkleProof,
    ) -> Result<(), Error>;

    /// Returns the current sequence number of the given externally owned account.
    async fn eoa_get_sequence(&self, address: HexSerializedVec) -> Result<u128, Error>;

    /// Returns the current balance of a particular fungible token in the given externally owned account.
    async fn eoa_get_fungible_token_balance(
        &self,
        address: HexSerializedVec,
        token_address: HexSerializedVec,
    ) -> Result<Decimal, Error>;

    /// Submits a transaction to transfer a fungible token
    /// from the given externally owned account to the given receiver address.
    async fn eoa_transfer_fungible_token(
        &self,
        address: HexSerializedVec,
        sender_private_key: HexSerializedVec,
        token_address: HexSerializedVec,
        receiver_address: HexSerializedVec,
        amount: Decimal,
    ) -> Result<(), Error>;
}
