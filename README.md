# kite-mesh

Peer-to-peer capability discovery and intent broadcast for the [Kite](https://github.com/Alpha-Centauri-Cyberspace) ecosystem.

> **Status:** pre-alpha. Code is actively being scaffolded; nothing here is stable yet.

## What is Kite Mesh?

Kite Mesh extends the Kite webhook delivery product with a libp2p-based peer-to-peer layer for:

- Publishing and discovering **capability cards** (what an agent can do)
- Broadcasting **intents** (what an agent is trying to do) to matching peers
- Running a distributed **directory** of capabilities with DHT-backed lookup
- Exchanging **receipts** with settlement and reputation

See [`MESH_PRD.md`](./MESH_PRD.md) for the full design — architecture, protocol, discovery algorithms, and cited evidence.

## Planned crates

This repo will hold a Cargo workspace containing:

| Crate | Role |
|---|---|
| `kite-mesh-core` | ACL envelope, capability cards, intents, receipts, identity, trust |
| `kite-mesh-transport` | libp2p behaviour, GossipSub, mDNS, Kademlia, relays |
| `kite-mesh-discovery` | Canonical encoder, facet fingerprints, LSH, directory, reranking |
| `kite-mesh-local-index` | Local search backends (flat scan, HNSW) |
| `kite-mesh-proto` | Protobuf definitions shared with `kite-protocol` |
| `kited` | Mesh daemon binary |
| `kite-mesh-research` | Feature-gated research-only code (non-shipping) |

## Relationship to other repos

- Wire-format types live in [`kite-protocol`](https://github.com/Alpha-Centauri-Cyberspace/kite-protocol) (crates.io).
- The operator-facing CLI is [`kite-cli`](https://github.com/Alpha-Centauri-Cyberspace/kite-cli); a `kite mesh` subcommand will talk to the daemon here.
- The relay server is [`kite-server`](https://github.com/Alpha-Centauri-Cyberspace/kite-server) (private).

## License

MIT — see [`LICENSE`](./LICENSE).
