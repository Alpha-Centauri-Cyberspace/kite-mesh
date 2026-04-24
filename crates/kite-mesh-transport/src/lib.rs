//! libp2p transport composition for Kite Mesh.
//!
//! The walking skeleton uses:
//! - TCP (random port) transport
//! - Noise XX handshake
//! - yamux multiplexing
//! - Kademlia DHT for directory records
//! - mDNS for Tier-1 local-subnet peer discovery
//! - identify for capability negotiation
//! - request-response (`AclCodec`) for ACL exchange and CapabilityCard fetch

#![allow(clippy::large_enum_variant)]

pub mod behaviour;
pub mod codec;
pub mod events;
pub mod swarm;

pub use behaviour::{MeshBehaviour, MeshBehaviourEvent};
pub use codec::{AclCodec, CardCodec};
pub use events::MeshEvent;
pub use swarm::build_swarm;
