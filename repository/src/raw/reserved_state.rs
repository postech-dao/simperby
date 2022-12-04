use anyhow::Error;
use reserved::ReservedState;
use simperby_common::*;
use std::fs;
use std::path::Path;

/// Reads the reserved state from the given path.
pub async fn read_reserved_state(path: &str) -> Result<ReservedState, Error> {
    let genesis_info =
        fs::read_to_string(format!("{}{}", path, "reserved/genesis_info.json")).unwrap();
    let genesis_info: GenesisInfo = serde_json::from_str(genesis_info.as_str()).unwrap();

    let mut members: Vec<Member> = vec![];
    let members_directory = fs::read_dir(format!("{}{}", path, "reserved/members")).unwrap();
    for file in members_directory {
        let path = file.unwrap().path();
        let path = path.to_str().unwrap();
        let member = fs::read_to_string(path).unwrap();
        let member: Member = serde_json::from_str(member.as_str()).unwrap();
        members.push(member);
    }

    let consensus_leader_order = fs::read_to_string(format!(
        "{}{}",
        path, "reserved/consensus_leader_order.json"
    ))
    .unwrap();
    let consensus_leader_order: Vec<usize> =
        serde_json::from_str(consensus_leader_order.as_str()).unwrap();

    let version = fs::read_to_string(format!("{}{}", path, "reserved/version")).unwrap();

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
    let genesis_info = serde_json::to_string(&state.genesis_info).unwrap();
    let consensus_leader_order = serde_json::to_string(&state.consensus_leader_order).unwrap();
    let version = serde_json::to_string(&state.version).unwrap();

    // Create files of reserved state.
    let path = Path::new(path).join("reserved");
    if !path.exists() {
        fs::create_dir(path.clone()).unwrap();
    }
    fs::write(path.join(Path::new("genesis_info.json")), genesis_info).unwrap();
    fs::write(
        path.join(Path::new("consensus_leader_order.json")),
        consensus_leader_order,
    )
    .unwrap();
    fs::write(path.join(Path::new("version")), version).unwrap();

    let path = path.join("members");
    if !path.exists() {
        fs::create_dir(path.clone()).unwrap();
    }
    for member in &state.members {
        let file_name = format!("{}{}", member.name, ".json");
        let member = serde_json::to_string(member).unwrap();
        fs::write(path.join(file_name.as_str()), member).unwrap();
    }

    Ok(())
}
