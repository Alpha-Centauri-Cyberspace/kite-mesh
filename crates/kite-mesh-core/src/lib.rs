//! Domain types and helpers for Kite Mesh.
//!
//! Network-free concerns: identity, canonical facet fingerprints, capability
//! cards, ACL envelope construction, and the receipt store.

pub mod acl;
pub mod capability;
pub mod error;
pub mod facets;
pub mod identity;
pub mod intent;
pub mod receipt;

pub use error::{Error, Result};
pub use identity::AgentIdentity;
