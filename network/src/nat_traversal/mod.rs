//! A module that provides NAT traversal for the node
//! which enables the node to be able to become a server.
//!
//! TODO: based on the libp2p's NAT traversal implementation,
//! describe the interface of this module so that it can be used with
//! the DMS module.
//!
//! For example, it could be a magical function that returns a port that can be
//! immediately bound to the socket.
//! Or it could require an interactive process that asks the user to perform some
//! complicated steps to configure the network.

use stun::agent::*;
use stun::message::*;
use stun::xoraddr::*;
use stun::Error;

use tokio::net::UdpSocket;
use tokio::time;

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::time::Duration;

// send_stun_request sends a request to the given stun addr using the given conn.
async fn send_stun_request(conn: Arc<UdpSocket>, addr: String) -> Result<(), Error> {
    let mut msg = Message::new();
    msg.build(&[Box::<TransactionId>::default(), Box::new(BINDING_REQUEST)])?;

    conn.send_to(&msg.raw, addr).await?;
    Ok(())
}

// keep_connecting_to_available_stun helps the given conn to maintain the public address
// by periodically sending a request to the available STUN server.
pub async fn keep_connecting_to_available_stun(
    conn: Arc<UdpSocket>,
    stun_cands_file_name: String,
    request_period: Duration,
) -> Result<(), Error> {
    loop {
        let file = File::open(stun_cands_file_name.clone())?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            match line {
                Ok(addr) => send_stun_request(conn.clone(), addr).await?,
                Err(err) => {
                    eprintln!("Error reading line: {}", err);
                }
            }
        }
        time::sleep(request_period).await;
    }
}

// keep_connecting_to_clients allows peers outside of NAT to persistently connect to this peer.
pub async fn keep_connecting_to_clients(
    conn: Arc<UdpSocket>,
    allowed_clients_file_name: String,
    request_period: Duration,
) -> Result<(), Error> {
    loop {
        let file = File::open(allowed_clients_file_name.clone())?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            match line {
                Ok(addr) => {
                    conn.send_to(b"DMS:NAT_TRAVERSAL:PING", addr).await?;
                }
                Err(err) => {
                    eprintln!("Error reading line: {}", err);
                }
            }
        }

        time::sleep(request_period).await;
    }
}

// decode_stun_response returns the public address by decoding the response from the stun server.
// Note(sejongk): You should broadcast your public address to other peers somehow for hole-punching.
pub fn decode_stun_response(data: &[u8]) -> Option<String> {
    let mut msg = Message::new();
    match msg.unmarshal_binary(data) {
        Ok(_) => {
            let mut xor_addr = XorMappedAddress::default();
            xor_addr.get_from(&msg).unwrap();
            Some(xor_addr.to_string())
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    use std::net::SocketAddr;

    #[tokio::test]
    async fn get_public_address_via_stun() {
        let bind_port = 8080;
        let bind_addr = SocketAddr::from(([0, 0, 0, 0], bind_port));
        let sock = UdpSocket::bind(bind_addr).await.unwrap();
        let conn = Arc::new(sock);

        let stun_addr = "stun.l.google.com:19302";
        send_stun_request(conn.clone(), stun_addr.to_string())
            .await
            .unwrap();

        let mut buf = [0; 1024];
        let (_len, _addr) = conn.recv_from(&mut buf).await.unwrap();
        let public_addr = decode_stun_response(&buf);
        assert!(public_addr.is_some());

        let re = Regex::new(r"^((25[0-5]|(2[0-4]|1\d|[1-9]|)\d)\.?\b){4}:\d{1,5}$").unwrap();
        assert!(re.is_match(&public_addr.unwrap()));
    }

    #[ignore]
    #[tokio::test]
    async fn run_server_behind_nat() {
        let bind_port = 8080;
        let bind_addr = SocketAddr::from(([0, 0, 0, 0], bind_port));
        let sock = UdpSocket::bind(bind_addr).await.unwrap();
        let conn = Arc::new(sock);

        let request_period = Duration::from_secs(10);
        let stun_cands_file = "stun_cands.txt".to_string();
        let allowed_clients_file = "allowed_clients.txt".to_string();

        tokio::spawn(keep_connecting_to_available_stun(
            conn.clone(),
            stun_cands_file,
            request_period,
        ));

        tokio::spawn(keep_connecting_to_clients(
            conn.clone(),
            allowed_clients_file,
            request_period,
        ));

        println!("Listening on: {}", conn.clone().local_addr().unwrap());
        let mut buf = [0; 1024];
        loop {
            match conn.recv_from(&mut buf).await {
                Ok((len, addr)) => match decode_stun_response(&buf) {
                    Some(public_addr) => {
                        println!("Public addr: {}", public_addr)
                    }
                    None => {
                        // Handle the DMS message
                        println!("{:?} bytes received from the Peer ({:?})", len, addr);
                        let len = conn.send_to(&buf[..len], addr).await.unwrap();
                        println!("{:?} bytes sent to the Peer ({:?})", len, addr);
                    }
                },
                Err(_) => {
                    println!("error!");
                }
            }
        }
    }

    #[ignore]
    #[tokio::test]
    async fn run_client_behind_nat() {
        let bind_port = 8080;
        let bind_addr = SocketAddr::from(([0, 0, 0, 0], bind_port));
        let sock = UdpSocket::bind(bind_addr).await.unwrap();
        let conn = Arc::new(sock);

        let request_period = Duration::from_secs(10);
        let stun_cands_file = "stun_cands.txt".to_string();

        tokio::spawn(keep_connecting_to_available_stun(
            conn.clone(),
            stun_cands_file,
            request_period,
        ));

        let server_addr = "127.0.0.1:8888".to_string();
        println!("Send a request to the peer: {}", server_addr);
        conn.send_to(b"DMS:CONSENSUS:PING", server_addr)
            .await
            .unwrap();

        let mut buf = [0; 1024];
        match conn.recv_from(&mut buf).await {
            Ok((len, addr)) => match decode_stun_response(&buf) {
                Some(public_addr) => {
                    println!("Public addr: {}", public_addr)
                }
                None => {
                    // Handle the DMS message
                    println!("{:?} bytes received from the Server ({:?})", len, addr);
                }
            },
            Err(_) => {
                println!("error!");
            }
        }
    }
}
