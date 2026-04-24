use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid identity key: {0}")]
    Identity(String),

    #[error("signature verification failed")]
    BadSignature,

    #[error("capability card failed validation: {0}")]
    InvalidCard(String),

    #[error("facet fingerprint mismatch")]
    FingerprintMismatch,

    #[error("embedding dimension mismatch: expected {expected}, got {got}")]
    EmbeddingDimension { expected: u32, got: u32 },

    #[error("unsupported encoder model: {0}")]
    UnsupportedModel(String),

    #[error("receipt store error: {0}")]
    Store(#[from] sqlx::Error),

    #[error("serialization error: {0}")]
    Serialize(#[from] prost::EncodeError),

    #[error("deserialization error: {0}")]
    Deserialize(#[from] prost::DecodeError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("libp2p identity error: {0}")]
    Libp2pIdentity(#[from] libp2p::identity::DecodingError),

    #[error("signing error: {0}")]
    Signing(#[from] libp2p::identity::SigningError),
}
