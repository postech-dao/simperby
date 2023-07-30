use super::*;
use crate::storage::StorageImpl;
use futures::future::join_all;
use rand::prelude::*;
use simperby_test_suite::*;

type Dms = super::dms::Dms<String>;

impl DmsMessage for String {
    const DMS_TAG: &'static str = "test_dms_message";

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

async fn create_dms(config: Config, private_key: PrivateKey) -> Dms {
    let path = create_temp_dir();
    StorageImpl::create(&path).await.unwrap();
    let storage = StorageImpl::open(&path).await.unwrap();
    Dms::new(storage, config, private_key).await.unwrap()
}

#[tokio::test]
async fn single_1() {
    let key = generate_random_string();
    let ((_, private_key), _, _) = setup_server_client_nodes(1).await;
    let mut dms = create_dms(
        Config {
            dms_key: key,
            members: vec![private_key.public_key()],
        },
        private_key,
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

pub async fn setup_server_client_nodes(
    client_n: usize,
) -> (
    (ServerNetworkConfig, PrivateKey),
    Vec<(ClientNetworkConfig, PrivateKey)>,
    Vec<PublicKey>,
) {
    let (_, server_private_key) = generate_keypair_random();
    let server = ServerNetworkConfig {
        port: dispense_port(),
    };
    let mut clients = Vec::new();
    for _ in 0..client_n {
        let (_, private_key) = generate_keypair_random();
        let network_config = ClientNetworkConfig {
            peers: vec![Peer {
                public_key: server_private_key.public_key(),
                name: "server".to_owned(),
                address: "127.0.0.1:1".parse().unwrap(),
                ports: vec![("dms-test_dms_message".to_owned(), server.port)]
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
    ((server, server_private_key), clients, pubkeys)
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
        Dms::sync(dms_, fetch_interval, broadcast_interval, network_config_)
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
    let key = "multi_1".to_owned();
    let ((server_network_config, server_private_key), client_network_config_and_keys, members) =
        setup_server_client_nodes(5).await;

    let server_dms = Arc::new(RwLock::new(
        create_dms(
            Config {
                dms_key: key.clone(),
                members: members.clone(),
            },
            server_private_key,
        )
        .await,
    ));
    let mut client_dmses = Vec::new();
    let mut tasks = Vec::new();

    let range_step = 10;
    for (i, (client_network_config, private_key)) in
        client_network_config_and_keys.iter().enumerate()
    {
        let dms = Arc::new(RwLock::new(
            create_dms(
                Config {
                    dms_key: key.clone(),
                    members: members.clone(),
                },
                private_key.clone(),
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
            Duration::from_millis(10000),
        ));
        client_dmses.push(dms);
    }
    tokio::spawn(Dms::serve(Arc::clone(&server_dms), server_network_config));
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
            (0..(range_step * client_network_config_and_keys.len()))
                .map(|x| format!("{x}"))
                .collect::<std::collections::BTreeSet<_>>(),
            messages
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        );
    }
}

async fn setup_client_nodes_with_specific_ports(
    client_n: usize,
    ports: Vec<u16>,
    pub_keys: Vec<PublicKey>,
) -> (Vec<(ClientNetworkConfig, PrivateKey)>, Vec<PublicKey>) {
    let mut clients = Vec::new();
    for _ in 0..client_n {
        let (_, private_key) = generate_keypair_random();
        let mut peers = Vec::new();

        for (port, key) in ports.iter().zip(pub_keys.iter()) {
            peers.push(Peer {
                public_key: key.clone(),
                name: "server".to_owned(),
                address: "127.0.0.1:1".parse().unwrap(),
                ports: vec![("dms-test_dms_message".to_owned(), *port)]
                    .into_iter()
                    .collect::<std::collections::BTreeMap<String, u16>>(),
                message: "".to_owned(),
                recently_seen_timestamp: 0,
            });
        }

        let network_config = ClientNetworkConfig { peers };
        clients.push((network_config, private_key));
    }
    let pubkeys = clients
        .iter()
        .map(|(_, private_key)| private_key.public_key())
        .chain(pub_keys.into_iter())
        .collect::<Vec<_>>();
    (clients, pubkeys)
}

#[tokio::test]
async fn multi_2() {
    let key = "multi_2".to_owned();
    // `server_a` and `server_b` each form a network topology with `client_a` and `client_b`, respectively.
    // `client_c` represents another network topology that connects both `server_a` and `server_b`.
    let (
        (server_a_network_config, server_a_private_key),
        client_a_network_config_and_keys,
        members_a,
    ) = setup_server_client_nodes(2).await;
    let (
        (server_b_network_config, server_b_private_key),
        client_b_network_config_and_keys,
        members_b,
    ) = setup_server_client_nodes(2).await;
    let (client_c_network_config_and_keys, members_c) = setup_client_nodes_with_specific_ports(
        1,
        vec![server_a_network_config.port, server_b_network_config.port],
        vec![
            server_a_private_key.public_key().clone(),
            server_b_private_key.public_key().clone(),
        ],
    )
    .await;

    let members = members_a
        .clone()
        .into_iter()
        .chain(members_b.clone().into_iter())
        .chain(members_c.clone().into_iter())
        .collect::<Vec<_>>();

    let server_a_dms = Arc::new(RwLock::new(
        create_dms(
            Config {
                dms_key: key.clone(),
                members: members.clone(),
            },
            server_a_private_key,
        )
        .await,
    ));

    let server_b_dms = Arc::new(RwLock::new(
        create_dms(
            Config {
                dms_key: key.clone(),
                members: members.clone(),
            },
            server_b_private_key,
        )
        .await,
    ));

    let mut tasks = Vec::new();
    let mut client_dmses = Vec::new();

    let client_network_config_and_keys = client_a_network_config_and_keys
        .iter()
        .chain(client_b_network_config_and_keys.iter())
        .chain(client_b_network_config_and_keys.iter())
        .chain(client_c_network_config_and_keys.iter())
        .collect::<Vec<_>>();

    let range_step = 10;
    for (i, (client_network_config, private_key)) in
        client_network_config_and_keys.iter().enumerate()
    {
        let dms = Arc::new(RwLock::new(
            create_dms(
                Config {
                    dms_key: key.clone(),
                    members: members.clone(),
                },
                private_key.clone(),
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
            Duration::from_millis(20000),
        ));
        client_dmses.push(dms);
    }
    tokio::spawn(Dms::serve(
        Arc::clone(&server_a_dms),
        server_a_network_config,
    ));
    tokio::spawn(Dms::serve(
        Arc::clone(&server_b_dms),
        server_b_network_config,
    ));
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
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            (0..(range_step * client_network_config_and_keys.len()))
                .map(|x| format!("{x}"))
                .collect::<std::collections::BTreeSet<_>>(),
            messages
        );
    }
}

#[tokio::test]
#[ignore]
async fn multi_3() {
    // TODO: test with the server turing off and on repeatedly.
    // clients must be able to sync with each other even if the server is not available 100% of the time.
}
