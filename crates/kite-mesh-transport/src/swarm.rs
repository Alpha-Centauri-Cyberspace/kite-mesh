//! Assemble a Kite Mesh `Swarm`.

use std::time::Duration;

use libp2p::{Swarm, SwarmBuilder, identity::Keypair, noise, tcp, yamux};

use crate::behaviour::MeshBehaviour;

/// Build a Tokio-backed swarm with TCP + Noise XX + yamux and the mesh
/// behaviour composition.
pub fn build_swarm(keypair: Keypair) -> anyhow::Result<Swarm<MeshBehaviour>> {
    let public_key = keypair.public();
    let local_peer_id = libp2p::PeerId::from_public_key(&public_key);

    let swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|_| {
            MeshBehaviour::new(local_peer_id, public_key)
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    Ok(swarm)
}
