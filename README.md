# kite-mesh

Peer-to-peer capability discovery and intent broadcast for the [Kite](https://github.com/Alpha-Centauri-Cyberspace) ecosystem.

> **Status:** pre-alpha. The walking skeleton is landing on the `feat/walking-skeleton` branch; most crates are still stubs.

## What is Kite Mesh?

Kite Mesh extends the Kite webhook delivery product with a libp2p-based peer-to-peer layer for:

- Publishing and discovering **capability cards** (what an agent can do)
- Broadcasting **intents** (what an agent is trying to do) to matching peers
- Running a distributed **directory** of capabilities with DHT-backed lookup
- Exchanging **receipts** with settlement and reputation

## The two documents you should read

| Doc | Purpose |
|---|---|
| [`MESH_PRD.md`](./MESH_PRD.md) | **North-star PRD** for the full platform. Architecture, protocol, discovery algorithms, phasing, and cited evidence. Read this to understand *where we're going*. |
| [`docs/walking_skeleton.md`](./docs/walking_skeleton.md) | **First-phase build plan.** What we're building right now: two daemons, mDNS + Kademlia + LSH directory + ACL round-trip + receipts. Read this to understand *what's happening this month*. |

## Getting started

Prerequisites: Rust stable (pinned via `rust-toolchain.toml`), macOS or Linux (mDNS relies on UDP multicast), ~100 MB free for the fastembed MiniLM model cache.

```sh
# Build the workspace.
cargo build --workspace

# Run the walking-skeleton integration test (first run downloads MiniLM-L6-v2 INT8).
cargo test --package kited --test walking_skeleton -- --nocapture

# Run a standalone daemon locally (Ctrl-C to stop).
cargo run --package kited -- run --config examples/local.toml
```

The integration test boots two daemons in-process and drives them through the full Phase 00 flow. Full prerequisites, what gets exercised, and how to clear caches: [`docs/walking_skeleton.md`](./docs/walking_skeleton.md).

## Workspace layout

| Crate | Status | Role |
|---|---|---|
| `kite-mesh-core` | stub | ACL envelope, capability cards, intents, receipts, identity, trust |
| `kite-mesh-transport` | stub | libp2p behaviour, GossipSub, mDNS, Kademlia, relays |
| `kite-mesh-discovery` | stub | Canonical encoder, facet fingerprints, LSH, directory, reranking |
| `kite-mesh-proto` | stub | Protobuf definitions shared with `kite-protocol` |
| `kited` | functional | Mesh daemon binary + `tests/walking_skeleton.rs` |
| `kite-mesh-local-index` | planned | Local search backends (flat scan, HNSW) |
| `kite-mesh-research` | planned | Feature-gated research-only code (non-shipping) |

## Relationship to other repos

- Wire-format types live in [`kite-protocol`](https://github.com/Alpha-Centauri-Cyberspace/kite-protocol) (crates.io).
- The operator-facing CLI is [`kite-cli`](https://github.com/Alpha-Centauri-Cyberspace/kite-cli); a `kite mesh` subcommand will talk to the daemon here.
- The relay server is [`kite-server`](https://github.com/Alpha-Centauri-Cyberspace/kite-server) (private).

## Contributing

- **Ideas and discussion:** open an issue on this repo. Reference the relevant section of `MESH_PRD.md` (e.g. "§6 Discovery") or the phase (e.g. "Phase 01") when you can — it makes threads much easier to follow.
- **Code:** `rustfmt` and `clippy` are enforced via [`.github/workflows/ci.yml`](./.github/workflows/ci.yml); CI also runs the full workspace build and test suite. Green CI is required before merge.
- **PRs:** keep them scoped to one concern. If your change touches architecture or protocol, link the relevant PRD section in the PR description so reviewers know which invariants you're working within.

## License

MIT — see [`LICENSE`](./LICENSE).
