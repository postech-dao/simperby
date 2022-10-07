use crate::*;
use serde::{Deserialize, Serialize};

/// A Merkle proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    // TODO
}

/// A light client state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightClient {
    pub repository_roots: Vec<Hash256>,
    pub state_roots_height_offset: u64,
    pub tx_roots: Vec<Hash256>,
    pub tx_roots_height_offset: u64,
    pub last_header: BlockHeader,
}

impl LightClient {
    /// Intializes a new light client with the initial header.
    pub fn new(initial_header: BlockHeader) -> Self {
        Self {
            repository_roots: vec![initial_header.repository_merkle_root.clone()],
            state_roots_height_offset: initial_header.height,
            tx_roots: vec![initial_header.tx_merkle_root.clone()],
            tx_roots_height_offset: initial_header.height,
            last_header: initial_header,
        }
    }

    /// Updates the header by providing the next block and the proof of it.
    pub fn update(&mut self, header: BlockHeader, proof: FinalizationProof) -> Result<(), String> {
        verify::verify_header_to_header(&self.last_header, &header).map_err(|e| e.to_string())?;
        verify::verify_finalization_proof(&header, &proof).map_err(|e| e.to_string())?;
        self.repository_roots
            .push(header.repository_merkle_root.clone());
        self.tx_roots.push(header.tx_merkle_root.clone());
        self.last_header = header;
        Ok(())
    }

    /// Verifies the given data with its proof.
    pub fn verify_commitment(
        &self,
        _message: Vec<u8>,
        _block_height: u64,
        _proof: MerkleProof,
    ) -> bool {
        unimplemented!()
    }
}
