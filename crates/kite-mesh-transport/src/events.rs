//! Flattened mesh events consumed by the daemon event loop.

use kite_mesh_proto::{AclMessage, CapabilityCard, DirectoryRecord};
use libp2p::PeerId;

/// High-level events the daemon's event loop reacts to.
#[derive(Debug)]
pub enum MeshEvent {
    /// A new peer was discovered (mDNS or Kademlia).
    PeerDiscovered { peer: PeerId },
    /// A local capability was successfully published to the DHT (records inserted).
    CapabilityPublished {
        capability_id: String,
        record_count: u32,
    },
    /// A DHT directory lookup returned a record matching an intent.
    DirectoryHit { record: Box<DirectoryRecord> },
    /// A fetched capability card was verified and is ready for reranking.
    CapabilityFetched { card: Box<CapabilityCard> },
    /// An incoming ACL request.
    AclRequest {
        from: PeerId,
        message: Box<AclMessage>,
    },
    /// An ACL response we had been awaiting.
    AclResponse {
        from: PeerId,
        message: Box<AclMessage>,
    },
    /// Transport listening on an address.
    Listening { address: String },
}
