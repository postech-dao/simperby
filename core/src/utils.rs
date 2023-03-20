use crate::Timestamp;

/// Generates a timestamp in the same as the node does.
pub fn get_timestamp() -> Timestamp {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as Timestamp
}
