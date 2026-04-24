# Walking Skeleton — Runbook

> **First-phase build plan** for Kite Mesh. The full-platform north star lives in [`MESH_PRD.md`](../MESH_PRD.md); this doc scopes Phase 00 only.

End-to-end validation of the Kite Mesh architecture: two local agents go from
fresh Ed25519 identities → mDNS discovery → Noise handshake → capability
publication via LSH directory → Kademlia lookup → ACL REQUEST/PROPOSE/AGREE/INFORM
→ signed receipts on both sides.

## Prerequisites

- Rust (stable, pinned via `rust-toolchain.toml`)
- macOS or Linux (mDNS relies on UDP multicast)
- ~100 MB free disk for the fastembed MiniLM model cache (`~/.cache/fastembed`)

## Three commands

```sh
# 1. Build the workspace.
cargo build --workspace

# 2. Run the integration test (first run downloads MiniLM-L6-v2 INT8).
cargo test --package kited --test walking_skeleton -- --nocapture

# 3. Run a standalone daemon locally (Ctrl-C to stop).
cargo run --package kited -- run --config examples/local.toml
```

The integration test boots two daemons in-process. After a warm fastembed
cache, the happy path closes in under a second end-to-end.

## What gets exercised

| Component | Proven by |
|---|---|
| Ed25519 identity | fresh keypair per daemon; all envelopes sign/verify |
| Noise XX transport | libp2p handshake at connection time |
| mDNS discovery | both daemons see each other's `PeerDiscovered` event |
| Kademlia DHT | `put_record` / `get_record` under `Quorum::One` |
| Angular LSH directory | `L = 4` directory records published per card; lookups hit |
| Canonical encoder | fastembed MiniLM-L6-v2-INT8, L2-normalized 384-dim |
| Facet fingerprint | deterministic across instances; cross-verified |
| Local rerank | exact facet filter + cosine dot-product |
| ACL round-trip | REQUEST → PROPOSE → AGREE → INFORM over libp2p request-response |
| Receipt store | SQLite row on each side, signatures verify |
| Metrics | four Prometheus counters advance; asserted inline |

## Observability

Each daemon exposes Prometheus metrics at `127.0.0.1:9191/metrics` by default
(configurable). Four required counters:

- `kite_mesh_directory_records_total`
- `kite_mesh_directory_lookup_seconds` (histogram)
- `kite_mesh_match_accept_total`
- `kite_mesh_receipts_total`

## Out of scope (see MESH_PRD.md §12 Phase 00)

GossipSub push, hash-prefix topics, HNSW local index, Tier 2 PSK / Tier 3
public swarm, relays, NAT traversal, reputation, payments, A2A/MCP/AGNTCY
bridges, CLI beyond `kited run`. Each of those lands in a later phase.

## Clearing caches

```sh
rm -rf ~/.cache/fastembed      # re-download MiniLM on next run
rm -rf target/                 # full workspace rebuild
```

## Running in CI

See `.github/workflows/ci.yml`. CI caches `~/.cache/fastembed` and the Cargo
registry; the first-ever CI run downloads the model (~27 MB), subsequent runs
hit the cache.
