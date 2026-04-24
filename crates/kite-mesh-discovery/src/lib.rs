//! Discovery layer: encoder, angular LSH directory, and local rerank.

pub mod directory;
pub mod encoder;
pub mod lsh;
pub mod rerank;

pub use encoder::{CanonicalEncoder, EncoderError};
pub use lsh::{AngularLsh, LshParams};
