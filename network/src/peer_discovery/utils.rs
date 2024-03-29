use super::*;
use eyre::eyre;
use libp2p::{
    identity::{self, ed25519},
    multiaddr::{Multiaddr, Protocol},
    PeerId,
};

/// Converts a simperby keypair into a libp2p keypair.
pub(crate) fn convert_keypair(
    pubkey: &PublicKey,
    privkey: &PrivateKey,
) -> Result<identity::Keypair, Error> {
    let mut keypair_bytes = privkey.as_ref().to_vec();
    keypair_bytes.extend(pubkey.as_ref());
    if let Ok(keypair_inner) = ed25519::Keypair::decode(&mut keypair_bytes) {
        Ok(identity::Keypair::Ed25519(keypair_inner))
    } else {
        Err(eyre!("not an ed25519 keypair"))
    }
}

pub(crate) fn convert_public_key(pubkey: &identity::PublicKey) -> Result<PublicKey, Error> {
    let identity::PublicKey::Ed25519(pubkey_inner) = pubkey;
    let bytes = pubkey_inner.encode();
    Ok(PublicKey::from_bytes(&bytes)?)
}

/// Converts to libp2p `PeerId`.
pub(crate) fn get_peer_id(peer: &Peer) -> Result<PeerId, Error> {
    if let Ok(libp2p_public_key) = ed25519::PublicKey::decode(peer.public_key.as_ref()) {
        Ok(identity::PublicKey::Ed25519(libp2p_public_key).to_peer_id())
    } else {
        Err(eyre!("not an ed25519 public key"))
    }
}

/// Converts libp2p Multiaddr into SocketAddrV4.
pub(crate) fn convert_multiaddr_into_sockv4(
    mut multiaddr: Multiaddr,
) -> Result<SocketAddrV4, Error> {
    let port = loop {
        if let Protocol::Tcp(port) = multiaddr
            .pop()
            .ok_or_else(|| eyre!("multiaddr does not contain a port"))?
        {
            break port;
        }
    };
    let ip = loop {
        if let Protocol::Ip4(ipv4_addr) = multiaddr
            .pop()
            .ok_or_else(|| eyre!("multiaddr does not contain an ipv4 address"))?
        {
            break ipv4_addr;
        }
    };
    Ok(SocketAddrV4::new(ip, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn peer_id_conversion() {
        let (public_key, private_key) = generate_keypair([1, 2, 3, 123]);
        let libp2p_keypair = convert_keypair(&public_key, &private_key).unwrap();
        let peer = Peer {
            public_key,
            address: "0.0.0.0:0".parse().unwrap(),
            ports: HashMap::new(),
            message: String::new(),
            recently_seen_timestamp: 0,
        };
        assert_eq!(
            get_peer_id(&peer).unwrap(),
            libp2p_keypair.public().to_peer_id()
        );
    }
}
