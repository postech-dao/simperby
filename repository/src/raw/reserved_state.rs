use anyhow::Error;
use reserved::ReservedState;
use simperby_common::*;
use std::path::Path;
use tokio::fs;

/// Reads the reserved state from the given path.
pub async fn read_reserved_state(path: &str) -> Result<ReservedState, Error> {
    let genesis_info =
        fs::read_to_string(format!("{}/{}", path, "reserved/genesis_info.json")).await?;
    let genesis_info: GenesisInfo = serde_json::from_str(genesis_info.as_str())?;

    let mut members: Vec<Member> = vec![];
    let mut members_directory = fs::read_dir(format!("{}/{}", path, "reserved/members")).await?;
    while let Some(member_file) = members_directory.next_entry().await? {
        let path = member_file.path();
        let member = fs::read_to_string(path).await?;
        let member: Member = serde_json::from_str(member.as_str())?;
        members.push(member);
    }

    let consensus_leader_order = fs::read_to_string(format!(
        "{}/{}",
        path, "reserved/consensus_leader_order.json"
    ))
    .await?;
    let consensus_leader_order: Vec<usize> = serde_json::from_str(consensus_leader_order.as_str())?;

    let version = fs::read_to_string(format!("{}/{}", path, "reserved/version")).await?;

    let reserved_state = ReservedState {
        genesis_info,
        members,
        consensus_leader_order,
        version,
    };

    Ok(reserved_state)
}

/// Writes the given reserved state to the given path, overwriting the existing file.
pub async fn write_reserved_state(path: &str, state: &ReservedState) -> Result<(), Error> {
    let genesis_info = serde_json::to_string(&state.genesis_info)?;
    let consensus_leader_order = serde_json::to_string(&state.consensus_leader_order)?;
    let version = serde_json::to_string(&state.version)?;

    // Create files of reserved state.
    let path = format!("{}/{}", path, "reserved");
    let reserved_path = Path::new(path.as_str());
    if !reserved_path.exists() {
        fs::create_dir(path.as_str()).await?;
    }

    fs::write(
        format!("{}/{}", path.as_str(), "genesis_info.json"),
        genesis_info,
    )
    .await?;
    fs::write(
        format!("{}/{}", path.as_str(), "consensus_leader_order.json"),
        consensus_leader_order,
    )
    .await?;
    fs::write(format!("{}/{}", path.as_str(), "version"), version).await?;

    let path = format!("{}/{}", path.as_str(), "members");
    let members_path = Path::new(path.as_str());
    if !members_path.exists() {
        fs::create_dir(path.as_str()).await?;
    }
    for member in &state.members {
        let file_name = format!("{}{}", member.name, ".json");
        let member = serde_json::to_string(member)?;
        fs::write(format!("{}/{}", path.as_str(), file_name), member).await?;
    }

    Ok(())
}
