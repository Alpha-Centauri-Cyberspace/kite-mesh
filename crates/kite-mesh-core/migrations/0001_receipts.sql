CREATE TABLE IF NOT EXISTS receipts (
    conversation_id     TEXT PRIMARY KEY,
    capability_id       TEXT NOT NULL,
    requester_peer_id   TEXT NOT NULL,
    provider_peer_id    TEXT NOT NULL,
    status              INTEGER NOT NULL,
    completed_at_ns     INTEGER NOT NULL,
    signature           BLOB NOT NULL,
    issuer_public_key   BLOB NOT NULL,
    counterparty_signed BLOB
);

CREATE INDEX IF NOT EXISTS idx_receipts_capability ON receipts (capability_id);
