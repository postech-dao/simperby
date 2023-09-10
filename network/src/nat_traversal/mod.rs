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

/*
//////////////////////////////////////////////////////////////
Here is an example for the server behind NAT.
//////////////////////////////////////////////////////////////

let bind_addr = SocketAddr::from(([0, 0, 0, 0], bind_port));
let sock = UdpSocket::bind(bind_addr).await?;
let conn = Arc::new(sock);

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

println!("Listening on: {}", conn.clone().local_addr()?);
let mut msg = Message::new();
let mut buf = [0; 1024];
loop {
    match conn.recv_from(&mut buf).await {
        Ok((len, addr)) => {
            if !is_stun_response(buf) {
                // Handle the DMS message.
            }
        },
        Err(_) => {
            println!("error!");
        }
    }
}
*/

/*
//////////////////////////////////////////////////////////////
Here is an example for the client behind NAT.
//////////////////////////////////////////////////////////////

let bind_addr = SocketAddr::from(([0, 0, 0, 0], bind_port));
let sock = UdpSocket::bind(bind_addr).await?;
let conn = Arc::new(sock);

tokio::spawn(keep_connecting_to_available_stun(
    conn.clone(),
    stun_cands_file,
    request_period,
));

println!("Send a request to the peer: {}", peer_addr);
conn.send_to(b"DMS:CONSENSUS:PING", peer_addr.clone())
    .await?;

match conn.recv_from(&mut buf).await {
    Ok((len, addr)) => {
        if !is_stun_response(buf) {
            // Handle the DMS message.
        }
    },
    Err(_) => {
        println!("error!");
    }
}
*/

// keep_connecting_to_available_stun helps the given conn keep the public address
// by periodically sending a request to the available STUN server.
pub async fn keep_connecting_to_available_stun(
    conn: Arc<UdpSocket>,
    stun_cands_file_name: String,
    request_period: Duration,
) -> Result<(), Error> {
    loop {
        let file = File::open(stun_cands_file_name.clone())?;
        let reader = BufReader::new(file);

        let mut msg = Message::new();
        msg.build(&[Box::<TransactionId>::default(), Box::new(BINDING_REQUEST)])?;

        for line in reader.lines() {
            match line {
                Ok(addr) => {
                    conn.send_to(&msg.raw, addr).await?;
                }
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

// is_stun_response returns whether the given data represents a response from the STUN server, and
// prints out the peer's current public address.
// You should broadcast your address to other peers somehow.
pub fn is_stun_response(data: &Vec<u8>) -> bool {
    let mut msg = Message::new();
    match msg.unmarshal_binary(&data) {
        Ok(_) => {
            let mut xor_addr = XorMappedAddress::default();
            xor_addr.get_from(&msg).unwrap();
            println!("Your current public address: {xor_addr}");
            return true;
        }
        Err(_) => {
            return false;
        }
    }
}
