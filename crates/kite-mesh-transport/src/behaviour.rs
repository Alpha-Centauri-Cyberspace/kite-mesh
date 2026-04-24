//! Combined NetworkBehaviour for Kite Mesh.

use std::time::Duration;

use libp2p::{
    PeerId, identify,
    kad::{self, store::MemoryStore},
    mdns,
    request_response::{self, ProtocolSupport},
    swarm::NetworkBehaviour,
};

use crate::codec::{ACL_PROTOCOL, AclCodec, CARD_PROTOCOL, CardCodec};

const IDENTIFY_PROTOCOL: &str = "/kite-mesh/id/1.0.0";
const AGENT_VERSION: &str = concat!("kited/", env!("CARGO_PKG_VERSION"));

#[derive(NetworkBehaviour)]
pub struct MeshBehaviour {
    pub kademlia: kad::Behaviour<MemoryStore>,
    pub mdns: mdns::tokio::Behaviour,
    pub identify: identify::Behaviour,
    pub acl: request_response::Behaviour<AclCodec>,
    pub card: request_response::Behaviour<CardCodec>,
}

impl MeshBehaviour {
    pub fn new(
        local_peer_id: PeerId,
        public_key: libp2p::identity::PublicKey,
    ) -> anyhow::Result<Self> {
        let kademlia = {
            let store = MemoryStore::new(local_peer_id);
            let mut cfg = kad::Config::new(kad::PROTOCOL_NAME);
            cfg.set_query_timeout(Duration::from_secs(10));
            let mut kad = kad::Behaviour::with_config(local_peer_id, store, cfg);
            // Server mode: accept incoming DHT put/get requests. The default
            // is client-only, which makes a small test mesh unable to store
            // records anywhere. See MESH_PRD.md §8 "Tier 1".
            kad.set_mode(Some(kad::Mode::Server));
            kad
        };

        let mdns = mdns::tokio::Behaviour::new(
            mdns::Config {
                ttl: Duration::from_secs(60),
                query_interval: Duration::from_secs(5),
                ..Default::default()
            },
            local_peer_id,
        )?;

        let identify = identify::Behaviour::new(
            identify::Config::new(IDENTIFY_PROTOCOL.into(), public_key)
                .with_agent_version(AGENT_VERSION.into()),
        );

        let acl = request_response::Behaviour::with_codec(
            AclCodec,
            std::iter::once((ACL_PROTOCOL, ProtocolSupport::Full)),
            request_response::Config::default().with_request_timeout(Duration::from_secs(10)),
        );

        let card = request_response::Behaviour::with_codec(
            CardCodec,
            std::iter::once((CARD_PROTOCOL, ProtocolSupport::Full)),
            request_response::Config::default().with_request_timeout(Duration::from_secs(5)),
        );

        Ok(Self {
            kademlia,
            mdns,
            identify,
            acl,
            card,
        })
    }
}
