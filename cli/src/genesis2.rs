use simperby_node::node::genesis;
use simperby_node::simperby_common::*;
use simperby_node::simperby_repository;
use simperby_test_suite::*;
use simperby_node::Config;

#[tokio::test]
async fn shit() {
    let rs = simperby_repository::raw::reserved_state::read_reserved_state(
        "/Users/junhayang/pdao/genesis/repository/repo",
    )
    .await
    .unwrap();
    println!("{}", rs.genesis_info.header.to_hash256());
    println!("{}", "7d86e58bc9f05726a7fc4b5a33feb82c535a3da8".to_owned().to_hash256());

    verify::verify_finalization_proof(&rs.genesis_info.header, &rs.genesis_info.genesis_proof).unwrap();
}

#[tokio::test]
async fn fuck() {
    let private = "4d922f9e9c69fa40dd2afd0922ee03425414a773b4f29b0f8e6d917f007a26a3";
    crate::genesis::setup_peer("/Users/junhayang/pdao/genesis", &[]).await;
    let private_key =
        PrivateKey::from_array(hex::decode(private).unwrap().as_slice().try_into().unwrap())
            .unwrap();
    genesis(Config {
        chain_name: "pdao-mainnet".to_owned(),
        public_key: private_key.public_key(),
        private_key,
        broadcast_interval_ms: None,
        fetch_interval_ms: None,
        public_repo_url: vec![],
        governance_port: 1155,
        consensus_port: 1166,
        repository_port: 1177,
    }, "/Users/junhayang/pdao/genesis").await.unwrap();
}

#[test]
fn sign_shit() {
    let private = "4d922f9e9c69fa40dd2afd0922ee03425414a773b4f29b0f8e6d917f007a26a3";

    let private =
        PrivateKey::from_array(hex::decode(private).unwrap().as_slice().try_into().unwrap())
            .unwrap();
    let sig = Signature::sign(
        Hash256::from_array(hex::decode("a4299aa7ff4855d68b0ac2ea2e1ab288c138692611e5f4366ff96c5cae0ecc9c").unwrap().as_slice().try_into().unwrap()),
        &private,
    )
    .unwrap();
    println!("{}", sig);
}
