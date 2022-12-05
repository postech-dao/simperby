use path_slash::PathExt as _;
use simperby_node::simperby_common::*;
use tempfile::TempDir;

#[cfg(target_os = "windows")]
pub async fn run_command(command: impl AsRef<str>) {
    println!("> RUN: {}", command.as_ref());
    let mut child = tokio::process::Command::new("C:/Program Files/Git/bin/sh.exe")
        .arg("--login")
        .arg("-c")
        .arg(command.as_ref())
        .spawn()
        .expect("failed to execute process");
    let ecode = child.wait().await.expect("failed to wait on child");
    assert!(ecode.success());
}

#[cfg(not(target_os = "windows"))]
pub async fn run_command(command: impl AsRef<str>) {
    println!("> RUN: {}", command.as_ref());
    let mut child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(command.as_ref())
        .spawn()
        .expect("failed to execute process");
    let ecode = child.wait().await.expect("failed to wait on child");
    assert!(ecode.success());
}

/// Generates a standard test chain config returning the genesis reserved-state
/// and the associated key pairs of the members.
pub fn generate_standard_genesis(
    member_number: usize,
) -> (ReservedState, Vec<(PublicKey, PrivateKey)>) {
    let keys = (0..member_number)
        .into_iter()
        .map(|i| generate_keypair(format!("{}", i)))
        .collect::<Vec<_>>();
    let members = keys
        .iter()
        .enumerate()
        .map(|(i, (public_key, _))| Member {
            public_key: public_key.clone(),
            // lexicographically ordered
            name: format!("member-{:04}", i),
            governance_voting_power: 1,
            consensus_voting_power: 1,
            governance_delegations: None,
            consensus_delegations: None,
        })
        .collect::<Vec<_>>();
    let genesis_header = BlockHeader {
        author: PublicKey::zero(),
        prev_block_finalization_proof: Vec::new(),
        previous_hash: Hash256::zero(),
        height: 0,
        timestamp: 0,
        commit_merkle_root: Hash256::zero(),
        repository_merkle_root: Hash256::zero(),
        validator_set: members
            .iter()
            .map(|member| (member.public_key.clone(), member.consensus_voting_power))
            .collect::<Vec<_>>(),
        version: "0.1.0".to_string(),
    };
    let genesis_info = GenesisInfo {
        header: genesis_header.clone(),
        genesis_proof: keys
            .iter()
            .map(|(_, private_key)| TypedSignature::sign(&genesis_header, private_key).unwrap())
            .collect::<Vec<_>>(),
        chain_name: "test-chain".to_string(),
    };
    (
        ReservedState {
            genesis_info,
            members,
            consensus_leader_order: (0..member_number).into_iter().collect::<Vec<_>>(),
            version: "0.1.0".to_string(),
        },
        keys,
    )
}

/// Creates a `repository` directory inside the given directory
/// and initializes a pre-genesis repository.
pub async fn setup_pre_genesis_repository(path: &str, reserved_state: ReservedState) {
    run_command(format!(
        "cd {} && mkdir repository && cd repository && git init",
        path
    ))
    .await;
    simperby_node::simperby_repository::raw::reserved_state::write_reserved_state(
        path,
        &reserved_state,
    )
    .await
    .unwrap();
    let path = format!("{}/repository", path);
    run_command(format!("cd {} && git add -A", path)).await;
    run_command(format!(
        "cd {} && git commit --author='TestAuthor <test@test.com>' -m 'genesis'",
        path
    ))
    .await;
}

pub fn create_temp_dir() -> String {
    let td = TempDir::new().unwrap();
    let path = td.path().to_slash().unwrap().into_owned();
    std::mem::forget(td);
    path
}
