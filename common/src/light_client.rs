use crate::*;
use merkle_tree::*;
use serde::{Deserialize, Serialize};

/// A light client state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightClient {
    pub repository_roots: Vec<Hash256>,
    pub state_roots_height_offset: u64,
    pub commit_roots: Vec<Hash256>,
    pub commit_roots_height_offset: u64,
    pub last_header: BlockHeader,
}

impl LightClient {
    /// Intializes a new light client with the initial header.
    pub fn new(initial_header: BlockHeader) -> Self {
        Self {
            repository_roots: vec![initial_header.repository_merkle_root],
            state_roots_height_offset: initial_header.height,
            commit_roots: vec![initial_header.commit_merkle_root],
            commit_roots_height_offset: initial_header.height,
            last_header: initial_header,
        }
    }

    /// Updates the header by providing the next block and the proof of it.
    pub fn update(&mut self, header: BlockHeader, proof: FinalizationProof) -> Result<(), String> {
        verify::verify_header_to_header(&self.last_header, &header).map_err(|e| e.to_string())?;
        verify::verify_finalization_proof(&header, &proof).map_err(|e| e.to_string())?;
        self.repository_roots.push(header.repository_merkle_root);
        self.commit_roots.push(header.commit_merkle_root);
        self.last_header = header;
        Ok(())
    }

    /// Verifies the given data with its proof.
    pub fn verify_commitment(
        &self,
        message: Vec<u8>,
        block_height: u64,
        proof: MerkleProof,
    ) -> bool {
        if block_height < self.commit_roots_height_offset
            || block_height >= self.commit_roots_height_offset + self.commit_roots.len() as u64
        {
            return false;
        }
        proof
            .verify(
                self.commit_roots[(block_height - self.commit_roots_height_offset) as usize],
                &message,
            )
            .is_ok()
    }
}
