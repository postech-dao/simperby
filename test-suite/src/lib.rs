use path_slash::PathExt as _;
use simperby_core::*;
use simperby_network::*;
use simperby_repository::raw::RawRepository;
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
    run_command(format!("cd {path} && git init")).await;
    let mut repository = RawRepository::open(path).await.unwrap();
    if !repository.check_gitignore().await.unwrap() {
        repository.commit_gitignore().await.unwrap();
    }

    simperby_repository::raw::reserved_state::write_reserved_state(path, &reserved_state)
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
                let mut v = (37000..38000).collect::<Vec<_>>();
                v.shuffle(&mut thread_rng());
                v
            })
        })
        .write()
        .pop()
        .expect("wtf did we have tests more than 1000?")
}

pub async fn create_test_dms<M: DmsMessage>(
    dms_key: String,
    members: Vec<PublicKey>,
    private_key: PrivateKey,
) -> Dms<M> {
    let path = create_temp_dir();
    StorageImpl::create(&path).await.unwrap();
    let storage = StorageImpl::open(&path).await.unwrap();
    Dms::new(storage, dms::Config { dms_key, members }, private_key)
        .await
        .unwrap()
}

pub async fn setup_server_client_nodes(
    dms_key: String,
    client_n: usize,
) -> (
    (ServerNetworkConfig, PrivateKey),
    Vec<(ClientNetworkConfig, PrivateKey)>,
    Vec<PublicKey>,
    FinalizationInfo,
) {
    let (fi, keys) = simperby_core::test_utils::generate_fi(client_n);
    let (_, server_private_key) = keys.last().unwrap().clone();
    let server = ServerNetworkConfig {
        port: dispense_port(),
    };
    let mut clients = Vec::new();
    for (_, private_key) in keys {
        let network_config = ClientNetworkConfig {
            peers: vec![Peer {
                public_key: server_private_key.public_key(),
                name: "server".to_owned(),
                address: "127.0.0.1:1".parse().unwrap(),
                ports: vec![(format!("dms-{dms_key}"), server.port)]
                    .into_iter()
                    .collect(),
                message: "".to_owned(),
                recently_seen_timestamp: 0,
            }],
        };
        clients.push((network_config, private_key));
    }
    let mut pubkeys = clients
        .iter()
        .map(|(_, private_key)| private_key.public_key())
        .collect::<Vec<_>>();
    pubkeys.push(server_private_key.public_key());
    ((server, server_private_key), clients, pubkeys, fi)
}

pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}
