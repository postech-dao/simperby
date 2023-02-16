use path_slash::PathExt as _;
use simperby_node::simperby_common::*;
use simperby_node::simperby_network::primitives::Storage;
use simperby_node::simperby_network::{
    dms, storage::StorageImpl, Dms, NetworkConfig, Peer, SharedKnownPeers,
};
use tempfile::TempDir;

pub fn setup_test() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        env_logger::init();
        color_eyre::install().unwrap();
    });
}

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

/// Creates a `repository` directory inside the given directory
/// and initializes a pre-genesis repository.
pub async fn setup_pre_genesis_repository(path: &str, reserved_state: ReservedState) {
    run_command(format!(
        "cd {path} && mkdir repository && cd repository && mkdir repo && cd repo && git init"
    ))
    .await;
    let path = format!("{path}/repository/repo");
    simperby_node::simperby_repository::raw::reserved_state::write_reserved_state(
        &path,
        &reserved_state,
    )
    .await
    .unwrap();
    println!("> Pre-genesis repository is created at {path}");

    run_command(format!("cd {path} && git add -A")).await;
    run_command(format!(
        "cd {path} && git config user.name 'Test' && git config user.email 'test@test.com'"
    ))
    .await;
    run_command(format!("cd {path} && git commit -m 'genesis'")).await;
}

pub async fn copy_repository(source_path: &str, dest_path: &str) {
    run_command(format!("mkdir -p {dest_path}/repository")).await;
    run_command(format!(
        "cp -r {source_path}/repository/repo {dest_path}/repository/repo"
    ))
    .await;
}

pub fn create_temp_dir() -> String {
    let td = TempDir::new().unwrap();
    let path = td.path().to_slash().unwrap().into_owned();
    std::mem::forget(td);
    path
}

/// Provides an available port (ranged from 37000 to 37999) for the test.
pub fn dispense_port() -> u16 {
    use once_cell::sync::OnceCell;
    static PORTS: OnceCell<parking_lot::RwLock<Vec<u16>>> = OnceCell::new();
    PORTS
        .get_or_init(|| {
            parking_lot::RwLock::new({
                use rand::seq::SliceRandom;
                use rand::thread_rng;
                let mut v = (37000..38000).into_iter().collect::<Vec<_>>();
                v.shuffle(&mut thread_rng());
                v
            })
        })
        .write()
        .pop()
        .expect("wtf did we have tests more than 1000?")
}

pub async fn create_test_dms(
    network_config: NetworkConfig,
    dms_key: String,
    peers: SharedKnownPeers,
) -> Dms {
    let path = create_temp_dir();
    StorageImpl::create(&path).await.unwrap();
    let storage = StorageImpl::open(&path).await.unwrap();
    Dms::new(
        storage,
        dms_key,
        dms::Config {
            fetch_interval: Some(std::time::Duration::from_millis(500)),
            broadcast_interval: Some(std::time::Duration::from_millis(500)),
            network_config,
        },
        peers,
    )
    .await
    .unwrap()
}

pub async fn setup_server_client_nodes(
    network_id: String,
    client_n: usize,
) -> (NetworkConfig, Vec<NetworkConfig>, SharedKnownPeers) {
    let (public_key, private_key) = generate_keypair_random();
    let server = NetworkConfig {
        network_id: network_id.clone(),
        ports: vec![(format!("dms-{network_id}"), dispense_port())]
            .into_iter()
            .collect(),
        members: Vec::new(),
        public_key,
        private_key,
    };
    let mut clients = Vec::new();
    for _ in 0..client_n {
        let (public_key, private_key) = generate_keypair_random();
        let network_config = NetworkConfig {
            network_id: network_id.clone(),
            ports: vec![(format!("dms-{network_id}"), dispense_port())]
                .into_iter()
                .collect(),
            members: Vec::new(),
            public_key,
            private_key: private_key.clone(),
        };
        clients.push(network_config);
    }
    let peer = SharedKnownPeers::new_static(vec![Peer {
        public_key: server.public_key.clone(),
        name: "server".to_owned(),
        address: "127.0.0.1:1".parse().unwrap(),
        ports: server.ports.clone(),
        message: "".to_owned(),
        recently_seen_timestamp: 0,
    }]);
    (server, clients, peer)
}

pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}
