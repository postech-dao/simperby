use anyhow::Error;
use reserved::ReservedState;
use simperby_common::*;

/// Reads the reserved state from the given path.
pub async fn read_reserved_state(_path: &str) -> Result<ReservedState, Error> {
    todo!()
}

/// Writes the given reserved state to the given path, overwriting the existing file.
pub async fn write_reserved_state(_path: &str, _state: &ReservedState) -> Result<(), Error> {
    todo!()
}
