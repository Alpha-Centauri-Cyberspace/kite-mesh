<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://getkite.sh/logo-on-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://getkite.sh/logo-on-light.svg">
    <img alt="Kite" src="https://getkite.sh/logo-on-dark.svg" width="220">
  </picture>

  <h3>P2P capability discovery and intent broadcasting for AI agents</h3>

  <p>
    <a href="#status"><img alt="Status: Pre-alpha" src="https://img.shields.io/badge/status-pre--alpha-00d4ff?style=flat-square&labelColor=0a0a0f"></a>
    <a href="./MESH_PRD.md"><img alt="PRD" src="https://img.shields.io/badge/PRD-MESH__PRD.md-00ff9d?style=flat-square&labelColor=0a0a0f"></a>
    <a href="https://getkite.sh"><img alt="Website" src="https://img.shields.io/badge/getkite.sh-00ff9d?style=flat-square&labelColor=0a0a0f"></a>
    <a href="./LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-e4e4e7?style=flat-square&labelColor=0a0a0f"></a>
  </p>
</div>

---

Kite Mesh is the **horizontal agent-to-agent discovery layer** — the thing that sits between MCP (tool access) and A2A (federation) and answers the question *"who out there can do the thing I need right now?"*.

Agents publish signed **capability cards**, broadcast **intents** as FIPA ACL speech acts, and negotiate execution over libp2p. Matching uses a canonical text encoder + angular LSH directory, with local reranking. No central registry, no vendor lock-in, local-first.

## How it fits

|                      | Scope                              | Examples                    |
| -------------------- | ---------------------------------- | --------------------------- |
| **MCP**              | How one agent calls a known tool   | Anthropic MCP               |
| **A2A / AGNTCY**     | How an agent federates to a peer   | Google A2A, AGNTCY          |
| **Kite Mesh** (this) | How agents **find** each other     | *this repo*                 |

The parent [Kite](https://getkite.sh) product ships webhook delivery today; Mesh is the substrate for agent collaboration we're building alongside it.

## Status

Phase 00 — walking skeleton. Implementation is live; the protocol and
directory semantics are stable enough to test end-to-end.

| Crate                      | Status                 | Role                                                                        |
| -------------------------- | ---------------------- | --------------------------------------------------------------------------- |
| `kite-mesh-proto`          | ✅ implemented          | prost-generated protobuf wire types (ACL envelopes, capability cards, ...)  |
| `kite-mesh-core`           | ✅ implemented          | Ed25519 identity, facet fingerprinting, ACL envelope signing, receipt store |
| `kite-mesh-transport`      | ✅ implemented          | libp2p behavior composition — Kademlia, mDNS, Noise, request-response       |
| `kite-mesh-discovery`      | ✅ implemented          | canonical encoder (MiniLM-L6-v2-INT8) + angular LSH + local rerank          |
| `kited`                    | ✅ implemented          | the mesh daemon binary                                                      |
| `kite-mesh-local-index`    | 🛠 Phase 1              | HNSW local index (flat scan in skeleton)                                    |
| `kite-mesh-research`       | 🧪 research             | projected cover / alignment certificates                                    |

Out of scope for Phase 00 and tracked in [`MESH_PRD.md`](./MESH_PRD.md):
GossipSub push, hash-prefix topics, PSK trust mesh, public bootstrap, A2A /
MCP / AGNTCY bridges.

## Run the walking skeleton

```
$ cp examples/local.toml kited.toml
$ cargo run -p kited -- run --config kited.toml
```

Two `kited` instances on the same LAN will discover each other via mDNS, publish capability cards to the Kademlia DHT, and exchange an intent round-trip end-to-end. See [`docs/walking_skeleton.md`](./docs/walking_skeleton.md) for the full runbook, expected logs, and Prometheus counters to watch (`directory_records_total`, `directory_lookup_seconds`, `match_accept_total`, `receipts_total`).

## Read the PRD

The full design — landscape positioning, protocol data model, matching algorithms, phasing, threat model — lives in [**`MESH_PRD.md`**](./MESH_PRD.md). Evidence-based, math where it matters, pointers to prior art. Start there if you want the *why*.

## Related

- **[kite-protocol](https://github.com/Alpha-Centauri-Cyberspace/kite-protocol)** — shared wire format.
- **[kite-cli](https://github.com/Alpha-Centauri-Cyberspace/kite-cli)** — the `kite` binary for webhook delivery.
- **[Kite](https://getkite.sh)** — managed event delivery platform.

## Contributing

This is early. The interesting contributions right now are around the directory (LSH parameter calibration, threshold policies), ACL envelope semantics, and failure-mode testing in the walking skeleton. Open an issue to chat before a big PR.

## License

MIT — see [`LICENSE`](./LICENSE).

---

<div align="center">
  <sub>
    <a href="https://getkite.sh">getkite.sh</a> ·
    <a href="https://github.com/Alpha-Centauri-Cyberspace">github</a> ·
    <a href="https://getkite.sh/docs">docs</a>
  </sub>
</div>
