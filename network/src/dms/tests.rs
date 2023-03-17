use super::*;
use crate::storage::StorageImpl;
use futures::future::join_all;
use rand::prelude::*;
use simperby_test_suite::*;

type Dms = super::dms::Dms<String>;

impl DmsMessage for String {
    fn check(&self) -> Result<(), Error> {
        Ok(())
    }
}

fn generate_random_string() -> String {
    let mut rng = rand::thread_rng();
    let s1: u128 = rng.gen();
    let s2: u128 = rng.gen();
    Hash256::hash(format!("{s1}{s2}").as_bytes()).to_string()[0..16].to_owned()
}

/// Returns the only-serving-node and the others, with the `Peer` info for the serving node.
/// `size` includes the serving node.
///
/// TODO: clients having themselves as a peer must be allowed.
fn generate_node_configs(
    serving_node_port: u16,
    size: usize,
) -> (
    ServerNetworkConfig,
    Vec<ClientNetworkConfig>,
    Vec<PublicKey>,
) {
    let mut client_configs = Vec::new();
    let mut keys = Vec::new();
    for _ in 0..size {
        keys.push(generate_keypair_random());
    }
    let network_id = generate_random_string();
    let server_peer = Peer {
        public_key: keys[0].0.clone(),
        name: format!("{}", keys[0].0),
        address: SocketAddrV4::new("127.0.0.1".parse().unwrap(), serving_node_port),
        ports: [(format!("dms-{network_id}"), serving_node_port)]
            .iter()
            .cloned()
            .collect(),
        message: "".to_owned(),
        recently_seen_timestamp: 0,
    };

    for i in 0..size - 1 {
        client_configs.push(ClientNetworkConfig {
            network_id: network_id.clone(),
            members: keys.iter().map(|(x, _)| x).cloned().collect(),
            private_key: keys[i + 1].1.clone(),
            peers: vec![server_peer.clone()],
        });
    }
    (
        ServerNetworkConfig {
            network_id: network_id.clone(),
            ports: [(format!("dms-{network_id}"), serving_node_port)]
                .iter()
                .cloned()
                .collect(),
            members: keys.iter().map(|(x, _)| x).cloned().collect(),
            private_key: keys[0].1.clone(),
        },
        client_configs,
        keys.into_iter().map(|(x, _)| x).collect(),
    )
}

async fn create_dms(config: Config, private_key: PrivateKey) -> Dms {
    let path = create_temp_dir();
    StorageImpl::create(&path).await.unwrap();
    let storage = StorageImpl::open(&path).await.unwrap();
    Dms::new(storage, config, private_key).await.unwrap()
}

#[tokio::test]
async fn single_1() {
    let key = generate_random_string();
    let network_config = generate_node_configs(dispense_port(), 1).0;
    let mut dms = create_dms(
        Config {
            dms_key: key,
            members: vec![network_config.private_key.public_key()],
        },
        network_config.private_key.clone(),
    )
    .await;

    for i in 0..10 {
        let msg = format!("{i}");
        dms.commit_message(&msg).await.unwrap();
    }

    let messages = dms
        .read_messages()
        .await
        .unwrap()
        .into_iter()
        .map(|x| x.message)
        .collect::<Vec<_>>();
    assert_eq!(
        (0..10)
            .map(|x| format!("{x}"))
            .collect::<std::collections::BTreeSet<_>>(),
        messages
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    );
}

async fn run_client_node(
    dms: Arc<RwLock<Dms>>,
    message_to_create: Vec<usize>,
    network_config: ClientNetworkConfig,
    broadcast_interval: Option<Duration>,
    fetch_interval: Option<Duration>,
    message_insertion_interval: Duration,
    final_sleep: Duration,
) {
    let dms_ = Arc::clone(&dms);
    let network_config_ = network_config.clone();
    let sync_task = tokio::spawn(async move {
        sync(dms_, fetch_interval, broadcast_interval, network_config_)
            .await
            .unwrap();
    });
    for i in message_to_create {
        tokio::time::sleep(message_insertion_interval).await;
        let msg = format!("{i}");
        dms.write().await.commit_message(&msg).await.unwrap();
    }
    tokio::time::sleep(final_sleep).await;
    sync_task.abort();
}

#[tokio::test]
async fn multi_1() {
    let (server_network_config, client_network_configs, members) =
        generate_node_configs(dispense_port(), 5);
    let key = server_network_config.network_id.clone();

    let server_dms = Arc::new(RwLock::new(
        create_dms(
            Config {
                dms_key: key.clone(),
                members: members.clone(),
            },
            server_network_config.private_key.clone(),
        )
        .await,
    ));
    let mut client_dmses = Vec::new();
    let mut tasks = Vec::new();

    let range_step = 10;
    for (i, client_network_config) in client_network_configs.iter().enumerate() {
        let dms = Arc::new(RwLock::new(
            create_dms(
                Config {
                    dms_key: key.clone(),
                    members: members.clone(),
                },
                client_network_config.private_key.clone(),
            )
            .await,
        ));
        tasks.push(run_client_node(
            Arc::clone(&dms),
            (i * range_step..(i + 1) * range_step).collect(),
            client_network_config.clone(),
            Some(Duration::from_millis(400)),
            Some(Duration::from_millis(400)),
            Duration::from_millis(50),
            Duration::from_millis(3000),
        ));
        client_dmses.push(dms);
    }
    tokio::spawn(serve(Arc::clone(&server_dms), server_network_config));
    join_all(tasks).await;

    for dms in client_dmses {
        let messages = dms
            .read()
            .await
            .read_messages()
            .await
            .unwrap()
            .into_iter()
            .map(|x| x.message)
            .collect::<Vec<_>>();
        assert_eq!(
            (0..(range_step * client_network_configs.len()))
                .map(|x| format!("{x}"))
                .collect::<std::collections::BTreeSet<_>>(),
            messages
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        );
    }
}
