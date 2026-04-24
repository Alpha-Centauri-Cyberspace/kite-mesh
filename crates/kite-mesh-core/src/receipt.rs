//! Receipt construction and SQLite persistence.

use std::path::Path;

use kite_mesh_proto::{Receipt, ResultStatus};
use prost::Message;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use crate::{error::Result, identity::AgentIdentity};

/// SQLite-backed receipt store.
#[derive(Clone)]
pub struct ReceiptStore {
    pool: SqlitePool,
}

impl ReceiptStore {
    /// Open (or create) the receipt store at `path`.
    pub async fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        sqlx::query(include_str!("../migrations/0001_receipts.sql"))
            .execute(&pool)
            .await?;
        Ok(Self { pool })
    }

    /// In-memory store used for tests.
    pub async fn in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .in_memory(true)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::query(include_str!("../migrations/0001_receipts.sql"))
            .execute(&pool)
            .await?;
        Ok(Self { pool })
    }

    pub async fn insert(
        &self,
        receipt: &Receipt,
        counterparty_signed: Option<&[u8]>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO receipts
                 (conversation_id, capability_id, requester_peer_id, provider_peer_id,
                  status, completed_at_ns, signature, issuer_public_key, counterparty_signed)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&receipt.conversation_id)
        .bind(&receipt.capability_id)
        .bind(&receipt.requester_peer_id)
        .bind(&receipt.provider_peer_id)
        .bind(receipt.status)
        .bind(receipt.completed_at_ns as i64)
        .bind(receipt.signature.as_ref())
        .bind(receipt.issuer_public_key.as_ref())
        .bind(counterparty_signed)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(&self, conversation_id: &str) -> Result<Option<StoredReceipt>> {
        type Row = (
            String,
            String,
            String,
            String,
            i32,
            i64,
            Vec<u8>,
            Vec<u8>,
            Option<Vec<u8>>,
        );
        let row: Option<Row> = sqlx::query_as(
            "SELECT conversation_id, capability_id, requester_peer_id, provider_peer_id,
                        status, completed_at_ns, signature, issuer_public_key, counterparty_signed
                   FROM receipts
                  WHERE conversation_id = ?",
        )
        .bind(conversation_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| StoredReceipt {
            receipt: Receipt {
                conversation_id: r.0,
                capability_id: r.1,
                requester_peer_id: r.2,
                provider_peer_id: r.3,
                status: r.4,
                completed_at_ns: r.5 as u64,
                signature: r.6.into(),
                issuer_public_key: r.7.into(),
            },
            counterparty_signature: r.8,
        }))
    }
}

#[derive(Debug, Clone)]
pub struct StoredReceipt {
    pub receipt: Receipt,
    pub counterparty_signature: Option<Vec<u8>>,
}

/// Build a signed Receipt.
pub fn build(
    identity: &AgentIdentity,
    conversation_id: String,
    capability_id: String,
    requester_peer_id: String,
    provider_peer_id: String,
    status: ResultStatus,
) -> Result<Receipt> {
    let mut receipt = Receipt {
        conversation_id,
        capability_id,
        requester_peer_id,
        provider_peer_id,
        status: status as i32,
        completed_at_ns: now_ns(),
        signature: Default::default(),
        issuer_public_key: identity.public_key().encode_protobuf().into(),
    };
    let bytes = signing_bytes(&receipt)?;
    receipt.signature = identity.sign(&bytes)?.into();
    Ok(receipt)
}

pub fn verify(receipt: &Receipt) -> Result<()> {
    let public =
        libp2p::identity::PublicKey::try_decode_protobuf(receipt.issuer_public_key.as_ref())
            .map_err(|e| crate::Error::InvalidCard(format!("bad issuer public key: {e}")))?;
    let bytes = signing_bytes(receipt)?;
    AgentIdentity::verify(&public, &bytes, receipt.signature.as_ref())
}

fn signing_bytes(receipt: &Receipt) -> Result<Vec<u8>> {
    let mut r = receipt.clone();
    r.signature = Default::default();
    let mut buf = Vec::with_capacity(r.encoded_len());
    r.encode(&mut buf)?;
    Ok(buf)
}

fn now_ns() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn insert_and_retrieve() {
        let id = AgentIdentity::generate();
        let store = ReceiptStore::in_memory().await.unwrap();
        let receipt = build(
            &id,
            "conv-1".into(),
            "cap-1".into(),
            id.peer_id().to_string(),
            "12D3KooWExample".into(),
            ResultStatus::Ok,
        )
        .unwrap();
        store.insert(&receipt, None).await.unwrap();
        let got = store.get("conv-1").await.unwrap().unwrap();
        assert_eq!(got.receipt.conversation_id, "conv-1");
        verify(&got.receipt).unwrap();
    }
}
