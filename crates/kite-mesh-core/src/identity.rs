//! Ed25519 agent identity.
//!
//! Thin wrapper around `libp2p::identity::Keypair`. The mesh is Ed25519-only
//! — other curves are unsupported in the walking skeleton.

use std::path::Path;

use libp2p::{PeerId, identity};
use tokio::fs;

use crate::error::{Error, Result};

#[derive(Clone)]
pub struct AgentIdentity {
    keypair: identity::Keypair,
}

impl AgentIdentity {
    /// Generate a fresh Ed25519 keypair.
    pub fn generate() -> Self {
        Self {
            keypair: identity::Keypair::generate_ed25519(),
        }
    }

    /// Load a keypair from a file of protobuf-encoded Ed25519 key bytes.
    pub async fn load(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = fs::read(path.as_ref()).await?;
        let keypair = identity::Keypair::from_protobuf_encoding(&bytes)?;
        Ok(Self { keypair })
    }

    /// Persist the keypair to `path`.
    pub async fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let bytes = self
            .keypair
            .to_protobuf_encoding()
            .map_err(|e| Error::Identity(e.to_string()))?;
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, bytes).await?;
        Ok(())
    }

    pub fn peer_id(&self) -> PeerId {
        PeerId::from_public_key(&self.keypair.public())
    }

    pub fn public_key(&self) -> identity::PublicKey {
        self.keypair.public()
    }

    pub fn keypair(&self) -> &identity::Keypair {
        &self.keypair
    }

    pub fn sign(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        Ok(self.keypair.sign(bytes)?)
    }

    /// Verify `signature` over `bytes` with the given peer's public key.
    pub fn verify(public: &identity::PublicKey, bytes: &[u8], signature: &[u8]) -> Result<()> {
        if public.verify(bytes, signature) {
            Ok(())
        } else {
            Err(Error::BadSignature)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn generate_sign_verify() {
        let id = AgentIdentity::generate();
        let msg = b"walking skeleton";
        let sig = id.sign(msg).unwrap();
        AgentIdentity::verify(&id.public_key(), msg, &sig).unwrap();
    }

    #[tokio::test]
    async fn save_and_load_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("keypair");
        let id = AgentIdentity::generate();
        id.save(&path).await.unwrap();
        let loaded = AgentIdentity::load(&path).await.unwrap();
        assert_eq!(id.peer_id(), loaded.peer_id());
    }
}
