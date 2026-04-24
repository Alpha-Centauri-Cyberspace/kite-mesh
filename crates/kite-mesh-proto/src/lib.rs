//! Generated prost types for the Kite Mesh wire protocol.
//!
//! All messages live in the single package `kite.mesh.v1` so cross-message
//! references resolve without prost's fragile super-path emission.

#![allow(clippy::large_enum_variant)]

pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/kite.mesh.v1.rs"));
}

pub use v1::*;

/// Wire protocol version. Bumped on any non-backward-compatible change to
/// serialized representations (capability fingerprints, DHT keys, signatures).
pub const PROTOCOL_VERSION: u32 = 1;

/// Canonical encoder id for the current production epoch.
pub const CANONICAL_MODEL_ID: &str = "all-minilm-l6-v2-int8";

/// Canonical embedding dimensionality.
pub const CANONICAL_EMBEDDING_DIM: u32 = 384;
