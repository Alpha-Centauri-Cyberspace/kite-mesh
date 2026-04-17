# Kite Agent Mesh — Deep Dive PRD

> **Status**: Draft v1.0  
> **Date**: April 12, 2026  
> **Author**: John Lomma + Claude  
> **Scope**: Open-source agent mesh network + Kite commercial integration

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Vision and Problem Statement](#vision-and-problem-statement)
3. [How This Relates to Kite Today](#how-this-relates-to-kite-today)
4. [Part 1 — The Open-Source Mesh (kite-mesh)](#part-1--the-open-source-mesh-kite-mesh)
5. [Part 2 — Kite as the First Commercial Client](#part-2--kite-as-the-first-commercial-client)
6. [Part 3 — Self-Hostable Backend (kite-meshd)](#part-3--self-hostable-backend-kite-meshd)
7. [Part 4 — The Open-Source Client Library (kite-mesh-sdk)](#part-4--the-open-source-client-library-kite-mesh-sdk)
8. [Architecture Deep Dive](#architecture-deep-dive)
9. [Competitive Landscape and Differentiation](#competitive-landscape-and-differentiation)
10. [Phasing and Milestones](#phasing-and-milestones)
11. [Open Questions and Risks](#open-questions-and-risks)
12. [Appendices](#appendices)

---

## Executive Summary

Kite today is a production-grade webhook adapter and event delivery system — it bridges external services (GitHub, Stripe, Linear) to AI agents and local dev environments using CloudEvents, WebSockets, and a CLI with 20+ commands and 8 sink types. It already has federation (P2P event routing between Kite instances), a billing layer (x402), and observability (Prometheus + Grafana).

The Kite Agent Mesh is the next evolutionary leap: a decentralized, intent-based communication layer where autonomous AI agents discover each other, negotiate capabilities, and collaborate — without a central coordinator. Think of it as going from "webhook plumbing" to "agent nervous system."

This document treats the mesh as **four distinct concerns**:

1. **The open-source mesh protocol and daemon** (`kite-mesh`) — stands completely on its own, no Kite dependency
2. **Kite commercial integration** — Kite becomes the first and best client on the mesh, bundling all existing functionality plus mesh capabilities into a paid product
3. **The self-hostable mesh backend** (`kite-meshd`) — a Rust binary that deploys with one click to Docker, Fly.io, Railway, or bare metal
4. **The open-source client SDK** (`kite-mesh-sdk`) — a Rust library any agent framework can use to join the mesh

---

## Vision and Problem Statement

### The World Today

AI agents (Claude Code, OpenClaw, Devin, Cursor, custom bots) operate in isolation. When they need to collaborate, the options are:

- **Centralized APIs**: Single point of failure, vendor lock-in, latency, cost. Every agent-to-agent call routes through a cloud service that extracts rent.
- **Ad-hoc integrations**: Direct HTTP calls, custom WebSocket protocols, bespoke message formats. No discoverability, no negotiation, no trust model.
- **Framework-locked orchestration**: CrewAI, AutoGen, LangGraph — powerful but walled gardens. An AutoGen agent can't talk to a CrewAI agent without a custom bridge.

### The World We're Building

A world where any agent can broadcast an intent ("I need someone who can execute Python in a sandboxed VM") and the mathematically closest capable agent on the network responds — whether it's on the same machine, a trusted friend's server, or the other side of the planet.

The mesh is to agents what TCP/IP was to computers: a universal communication layer that doesn't care who built the endpoints.

### Why This Matters for Kite

Kite already solves the "events → agents" problem. The mesh solves the "agents → agents" problem. Together, they create a complete event-driven agent platform where external events trigger agent workflows that can fan out across a mesh of collaborators.

---

## How This Relates to Kite Today

### What Kite Already Has (and the Mesh Leverages)

Kite's existing architecture is not throwaway — it's infrastructure the mesh builds on top of:

| Existing Kite Capability | Mesh Relevance |
|---|---|
| **CloudEvents v1.0 envelope** | The mesh ACL (Agent Communication Language) extends CloudEvents with performative/ontology fields. Zero format conflict. |
| **WebSocket broadcast (per-team)** | State Channels evolve this into persistent, bi-directional agent-to-agent streams. The broadcast topology becomes a fallback when P2P isn't available. |
| **Federation (P2P event routing)** | Federation is literally a centralized version of what the mesh does decentralized. The outbox worker, delivery tracking, loop detection, and retry logic all port directly. |
| **Kite CLI sinks (8 types)** | The mesh daemon becomes a new sink type. `kite stream --sink mesh` bridges existing webhook events onto the mesh. |
| **API key auth + team scoping** | Trust-Mesh (Tier 2) uses Pre-Shared Keys — Kite's API key infrastructure provides the management layer for PSK generation, rotation, and revocation. |
| **x402 payment metering** | Global Swarm (Tier 3) needs payment rails for "hire an agent to scrape 10K URLs." x402 is already integrated. |
| **SQLite DLQ + retry** | The mesh client needs exactly this for offline message queuing when peers are unreachable. |
| **Prometheus + Grafana observability** | Mesh telemetry (peer count, routing latency, capability match rates) plugs directly into existing dashboards. |
| **Device auth flow** | Agent enrollment on the mesh can reuse the device code → browser approval → key issuance flow. |
| **Skill registry** | Capability Cards are the mesh equivalent of skills. The existing `kite skill install/export` commands map to capability card broadcast/discovery. |

### What Kite Does NOT Have (and Needs)

| Gap | Required For |
|---|---|
| **libp2p transport** | Peer discovery, NAT traversal, GossipSub, circuit relays |
| **Local embedding engine** | Semantic routing — vectorizing capability cards and intents |
| **HNSW vector index** | In-memory nearest-neighbor search for capability matching |
| **Agent Communication Language** | Structured negotiation protocol (request/propose/agree/inform) |
| **State Channel management** | Dedicated bi-directional streams after agent matching |
| **mDNS discovery** | Zero-config local subnet agent discovery |
| **Kademlia DHT participation** | Internet-wide agent discovery (Trust-Mesh and Global Swarm tiers) |
| **Capability Card schema** | Standardized agent self-description format |

---

## Part 1 — The Open-Source Mesh (kite-mesh)

### Design Philosophy

The mesh protocol is **completely independent of Kite**. It has no Kite dependencies, no Kite imports, no Kite branding in the core library. Someone should be able to `cargo add kite-mesh` and build an agent mesh without ever knowing Kite exists.

This is critical for adoption. The mesh wins by being ubiquitous, and ubiquity requires neutrality.

### Repository Structure

```
kite-mesh/                          # Standalone repo: github.com/kitemesh/kite-mesh
├── Cargo.toml                      # Workspace root
├── LICENSE                         # Apache-2.0 OR MIT (dual license)
├── README.md
├── crates/
│   ├── kite-mesh-core/             # Core types, traits, ACL definitions
│   │   ├── src/
│   │   │   ├── acl.rs              # Agent Communication Language types
│   │   │   ├── capability.rs       # Capability Card schema + validation
│   │   │   ├── intent.rs           # Intent broadcast types
│   │   │   ├── identity.rs         # Agent identity (Ed25519 keypairs)
│   │   │   ├── trust.rs            # Trust signatures, PSK management
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   │
│   ├── kite-mesh-transport/        # libp2p transport layer
│   │   ├── src/
│   │   │   ├── behaviour.rs        # Combined NetworkBehaviour
│   │   │   ├── gossipsub.rs        # GossipSub configuration
│   │   │   ├── mdns.rs             # Local subnet discovery
│   │   │   ├── kademlia.rs         # DHT for internet-wide discovery
│   │   │   ├── relay.rs            # Circuit relay for NAT traversal
│   │   │   ├── state_channel.rs    # Dedicated P2P streams post-match
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   │
│   ├── kite-mesh-routing/          # Semantic routing engine
│   │   ├── src/
│   │   │   ├── embedder.rs         # ONNX/Candle embedding inference
│   │   │   ├── hnsw.rs             # In-memory HNSW index
│   │   │   ├── vector_dht.rs       # Semantic DHT overlay
│   │   │   ├── matcher.rs          # Intent → Capability matching
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   │
│   ├── kite-mesh-proto/            # Wire format definitions
│   │   ├── proto/
│   │   │   ├── acl.proto           # ACL message format
│   │   │   ├── capability.proto    # Capability Card schema
│   │   │   ├── routing.proto       # Routing metadata
│   │   │   └── state_channel.proto # State channel frames
│   │   ├── src/lib.rs              # Generated code (prost)
│   │   └── Cargo.toml
│   │
│   └── kite-mesh-daemon/           # The sidecar binary
│       ├── src/
│       │   ├── main.rs
│       │   ├── api.rs              # Local HTTP/gRPC API for host agent
│       │   ├── config.rs           # TOML/env configuration
│       │   └── metrics.rs          # Prometheus metrics
│       └── Cargo.toml
│
├── examples/
│   ├── local_swarm/                # Two agents on localhost
│   ├── trust_mesh/                 # PSK-protected multi-host
│   └── capability_discovery/       # Semantic routing demo
│
└── tests/
    ├── integration/
    └── e2e/
```

### Core Protocol Design

#### Agent Communication Language (ACL)

The ACL is the mesh's lingua franca. Every message on the mesh is an ACL envelope:

```protobuf
message AclMessage {
  // Identity
  string sender_id = 1;          // Ed25519 public key (base58)
  bytes signature = 2;            // Ed25519 signature of payload
  uint64 timestamp_ns = 3;       // Nanosecond Unix timestamp
  string message_id = 4;         // UUIDv7 (time-sortable)
  
  // Routing
  Performative performative = 5;
  string ontology = 6;           // Domain context (e.g., "code-execution", "web-scraping")
  string conversation_id = 7;    // Groups related messages
  string in_reply_to = 8;        // References previous message_id
  
  // Payload
  oneof payload {
    IntentPayload intent = 10;
    ProposalPayload proposal = 11;
    AgreementPayload agreement = 12;
    ResultPayload result = 13;
    StreamFrame stream = 14;
    CapabilityCard capability = 15;
  }
  
  // Trust
  TrustContext trust = 20;
}

enum Performative {
  REQUEST = 0;      // "I need something"
  PROPOSE = 1;      // "I can do that, here's my offer"
  AGREE = 2;        // "Deal, let's proceed"
  REFUSE = 3;       // "Can't help with that"
  INFORM = 4;       // "Here's the result"
  CANCEL = 5;       // "Never mind"
  FAILURE = 6;      // "I tried but failed"
  SUBSCRIBE = 7;    // "Notify me when..."
}
```

#### Capability Cards

Every agent on the mesh publishes a Capability Card — a structured, signable description of what it can do:

```protobuf
message CapabilityCard {
  string agent_id = 1;
  string agent_name = 2;
  string agent_version = 3;
  
  // What this agent can do
  repeated Tool tools = 4;
  repeated string ontologies = 5;      // Domains: ["code-execution", "data-analysis"]
  
  // Constraints
  ResourceLimits limits = 6;
  PricingModel pricing = 7;            // Free, per-task, per-minute, etc.
  
  // Trust
  bytes public_key = 8;
  repeated TrustSignature endorsements = 9;  // Signed by trusted peers
  
  // Metadata for semantic matching
  string description = 10;            // Human-readable description
  bytes embedding = 11;               // Pre-computed vector (384-dim float16)
  
  // Availability
  AgentStatus status = 12;            // AVAILABLE, BUSY, OFFLINE
  uint64 last_seen_ns = 13;
}

message Tool {
  string name = 1;                     // e.g., "python_exec", "bash", "web_scrape"
  string description = 2;
  repeated string input_schemas = 3;   // JSON Schema references
  repeated string output_schemas = 4;
}
```

#### Semantic Routing: How Intent Matching Works

This is the core innovation. Traditional DHTs route by key hash — you look up a specific key and find the node responsible for it. A Semantic DHT routes by meaning — you describe what you need and find the node whose capabilities are closest in embedding space.

**The Flow:**

1. **Agent A** broadcasts an intent: "I need to execute a Python script in a sandboxed Linux environment with GPU access"
2. The local mesh daemon **embeds** this intent using a quantized `all-MiniLM-L6-v2` model (384-dimensional vector, ~22MB model file, <5ms inference on CPU)
3. The daemon queries its local **HNSW index** of known capability card embeddings
4. If a local match exists (cosine similarity > 0.85 threshold), it routes directly
5. If not, the intent metadata (NOT the full payload) is broadcast via **GossipSub**
6. Remote peers check their local HNSW indices and respond if they match
7. The **Vector DHT** provides a global fallback — capability embeddings are stored in a Kademlia-like DHT where the "key" is the embedding vector and the "value" is the agent's PeerId

**Why This Avoids Broadcast Storms:**

Traditional pub/sub broadcasts everything to everyone. The semantic approach means:

- GossipSub only carries lightweight metadata (intent hash + embedding vector + ontology tag) — approximately 500 bytes per gossip message
- Peers only request the full payload if their local HNSW search returns a match above the similarity threshold
- The Vector DHT provides directed lookups when gossip doesn't surface a match within a configurable timeout (default 200ms for local, 2s for internet)

**Embedding Engine Details:**

We use ONNX Runtime via the `ort` crate (production-ready, approaching v2.0 GA) or Candle (Hugging Face's Rust ML framework) for local embedding inference:

- **Model**: `all-MiniLM-L6-v2` quantized to INT8 — 22MB file, 384-dimensional output
- **Inference**: <5ms on modern CPU, <2ms on Apple Silicon, negligible GPU
- **Memory**: ~50MB resident including model + runtime
- **Startup**: <500ms cold start (model load + ONNX session creation)
- **Alternative**: Candle compiled via `candle-transformers` — slightly smaller footprint, no external runtime dependency, but less hardware accelerator support

The embedding engine runs **once at startup** to vectorize the local agent's capability card, and **on-demand** for incoming intents. Embeddings are cached aggressively — the same intent text always produces the same vector.

### Network Topology Design

#### Tier 1: Local Subnet (mDNS)

**Mechanism**: Zero-config peer discovery using multicast DNS on the local network. No internet required, no configuration required.

**Technical Implementation:**

- libp2p `mdns::tokio::Behaviour` with a 30-second discovery interval
- Peers announce on `_kite-mesh._udp.local`
- Capability cards exchanged on first peer contact via a dedicated `/kite-mesh/capability/1.0.0` protocol
- HNSW index updated incrementally as peers join/leave

**Use Cases:**

- Local dev swarm: A Claude Code instance offloads sandboxed execution to a Docker-hosted agent on the same machine
- Home lab: Your NAS-hosted agent collaborates with your desktop agent
- Office network: Team agents discover each other automatically

**Performance targets:**

- Peer discovery: <2 seconds from daemon start to first peer contact
- Intent matching: <10ms end-to-end (embed + HNSW lookup + GossipSub round-trip on LAN)
- State channel establishment: <50ms after match

#### Tier 2: Trust-Mesh (PSK Layer)

**Mechanism**: A permissioned overlay network for trusted peers across the internet. Uses Pre-Shared Keys to authenticate peers and circuit relays for NAT traversal.

**Technical Implementation:**

- PSKs generated via Kite's API key management UI (or `kite-mesh keygen` for standalone users)
- libp2p `pnet::PreSharedKeyConfig` for transport-level encryption
- Circuit relay nodes for peers behind NATs (configurable: use public relays or self-host)
- Kademlia DHT scoped to the trust group (bootstrap nodes = the peers themselves)

**Trust Model:**

Capability cards on the Trust-Mesh carry endorsement signatures. If Agent A trusts Agent B, and Agent B endorses Agent C's capability card, Agent A can transitively trust Agent C up to a configurable depth (default: 2 hops).

```
Trust Chain: A → B → C  (depth 2, accepted)
             A → B → C → D  (depth 3, rejected by default)
```

**Use Cases:**

- Family agent coordination: Your scheduling agent negotiates with a friend's family agent for playdates
- Team collaboration: Distributed team agents share context and delegate tasks
- Multi-org workflows: Agency agents collaborate with client agents on shared projects

#### Tier 3: Global Swarm (Public DHT)

**Mechanism**: The permissionless public layer. Anyone can join, capabilities are discoverable globally, and payment rails handle compensation.

**Technical Implementation:**

- Public Kademlia DHT with well-known bootstrap nodes (we operate initial set, community can add)
- Capability cards published to DHT with reputation scores (derived from successful task completions)
- x402 payment integration for paid task execution
- Proof-of-work or stake-based spam prevention on capability registration

**Trust Model:**

No inherent trust. Reputation is earned. New agents start with a neutral score. Successful task completions (verified by the requester's signed receipt) increase reputation. Failed tasks decrease it.

**Use Cases:**

- "I need someone to scrape 10K URLs cheaply" — routed to the most cost-effective agent globally
- "I need a GPU-equipped agent to run this ML inference" — discovery of specialized compute agents
- Open-source agent marketplace — agents offer services, requesters pay per-task

---

## Part 2 — Kite as the First Commercial Client

### The Pitch

Kite becomes the "Tailscale of agent mesh" — the easiest, most polished way to get agents onto the mesh with enterprise-grade management, observability, and billing.

The open-source mesh is the protocol. Kite is the product.

### What Kite Adds on Top of the Open Mesh

#### 2.1 Managed Trust Infrastructure

**Today**: Kite manages API keys, team scoping, and device auth flows.

**With Mesh**: Kite manages PSK lifecycle (generation, rotation, revocation), trust group membership, endorsement chains, and provides a dashboard for visualizing your trust graph.

**Implementation:**

- New `trust_groups` table in Postgres (extends existing `teams` schema)
- PSK generation via Ed25519 key derivation from team secrets
- Dashboard page: "Trust Network" — visualize which agents trust each other, endorsement chains, trust depth
- CLI: `kite mesh trust add <peer_id>`, `kite mesh trust revoke <peer_id>`, `kite mesh trust list`

#### 2.2 Webhook → Mesh Bridge

**Today**: Kite receives webhooks and delivers them to CLI sinks (stdout, proxy, socket, exec, MCP).

**With Mesh**: Kite becomes a webhook-to-mesh gateway. External events (GitHub push, Stripe payment, Linear issue) automatically broadcast as mesh intents, routable to any agent on your trust network.

**Implementation:**

- New sink type: `MeshSink` in `crates/kite-cli/src/sinks/`
- Configuration: `kite stream --sink mesh --ontology github-events`
- Transform layer: CloudEvent → ACL message with `performative: INFORM` and `ontology` derived from source
- Filtering: Only events matching configured intent patterns are bridged (not a firehose)

#### 2.3 Agent Fleet Management Dashboard

**Today**: Kite dashboard shows event timelines, API keys, endpoints, and billing.

**With Mesh**: Dashboard adds fleet management — see all your agents, their capabilities, status, mesh topology, and real-time collaboration streams.

**New Dashboard Pages:**

- **Fleet Overview**: All connected agents, their capability cards, online/offline status, current tasks
- **Mesh Topology**: Real-time network graph (local subnet, trust-mesh, global swarm connections)
- **Task History**: Every intent broadcast, match, negotiation, and result — with latency metrics
- **Capability Registry**: Browse and manage what your agents advertise to the mesh
- **Cost Center**: For Global Swarm tasks — track spend on external agent services (x402 integration)

#### 2.4 Enhanced Observability

**Today**: Prometheus + Grafana for webhook delivery metrics.

**With Mesh**: Full mesh telemetry — peer discovery rates, semantic match quality, routing latency histograms, GossipSub message volumes, state channel utilization.

**New Metrics:**

```
kite_mesh_peers_total{tier="local|trust|global"}
kite_mesh_intent_broadcasts_total{ontology="..."}
kite_mesh_matches_total{ontology="...", similarity_bucket="0.85-0.90|0.90-0.95|0.95-1.0"}
kite_mesh_match_latency_seconds{tier="local|trust|global", quantile="0.5|0.95|0.99"}
kite_mesh_state_channels_active
kite_mesh_gossipsub_messages_total{direction="in|out"}
kite_mesh_embedding_inference_seconds{quantile="0.5|0.95|0.99"}
kite_mesh_capability_cards_indexed
```

#### 2.5 CLI Extensions

New commands added to the existing Kite CLI:

```bash
# Mesh daemon management
kite mesh start                      # Start the sidecar daemon
kite mesh stop                       # Stop the daemon
kite mesh status                     # Show peer count, tier connectivity, local agent info

# Capability management
kite mesh cap publish                # Broadcast capability card to mesh
kite mesh cap list                   # List discovered capabilities on the mesh
kite mesh cap search "python sandboxed execution"  # Semantic search for capabilities

# Intent operations
kite mesh intent broadcast "I need X"  # Send an intent to the mesh
kite mesh intent watch                 # Stream incoming intents

# Trust management
kite mesh trust keygen               # Generate PSK for trust group
kite mesh trust add <peer_id>        # Add peer to trust group
kite mesh trust list                 # Show trust graph

# Debugging
kite mesh peers                      # List connected peers with latency
kite mesh topology                   # Show network topology
kite mesh logs                       # Stream daemon logs
```

#### 2.6 Pricing Model

| Tier | Mesh Features | Price |
|---|---|---|
| **Free (Open Source)** | Local Subnet only, 5 peers, no dashboard | $0 |
| **Starter** | All tiers, 25 peers, basic dashboard, 1K events/day mesh bridge | $29/mo |
| **Pro** | All tiers, unlimited peers, full dashboard, fleet management, 50K events/day | $99/mo |
| **Enterprise** | Self-hosted option, custom trust policies, SLA, audit logs, SSO | Custom |

The open-source mesh itself is always free. Kite charges for the management layer, dashboard, enhanced observability, and webhook-to-mesh bridge.

---

## Part 3 — Self-Hostable Backend (kite-meshd)

### Design Principles

1. **Single binary**: One Rust binary, statically compiled, <20MB
2. **Zero mandatory dependencies**: Runs standalone with embedded SQLite for state. Postgres optional for scale.
3. **One-click deploy**: Docker image, Fly.io template, Railway template, Shuttle config, bare metal script
4. **Configuration by environment**: Everything configurable via env vars or TOML, sensible defaults for everything
5. **Horizontal scale**: Multiple instances discover each other via the mesh itself (dogfooding)

### Architecture

```
┌─────────────────────────────────────────────┐
│              kite-meshd binary               │
│                                              │
│  ┌──────────┐  ┌───────────┐  ┌───────────┐ │
│  │ libp2p   │  │ Embedding │  │ Local API │ │
│  │ Runtime  │  │ Engine    │  │ (HTTP)    │ │
│  │          │  │           │  │           │ │
│  │ • mDNS   │  │ • ONNX RT │  │ • REST    │ │
│  │ • Gossip │  │ • HNSW    │  │ • gRPC    │ │
│  │ • Kadmla │  │ • Cache   │  │ • WS      │ │
│  │ • Relay  │  │           │  │ • Metrics │ │
│  └──────────┘  └───────────┘  └───────────┘ │
│                                              │
│  ┌──────────────────────────────────────────┐│
│  │          State Store (SQLite/Postgres)    ││
│  │  • Peer table    • Capability index      ││
│  │  • Trust graph   • Message log           ││
│  │  • Config cache  • DLQ                   ││
│  └──────────────────────────────────────────┘│
└─────────────────────────────────────────────┘
         │              │              │
    P2P Traffic    Host Agent     Prometheus
    (TCP/QUIC)     (localhost)    (scrape)
```

### Build Strategy

**Language**: Rust (consistent with Kite's existing backend)

**Why Rust over Go/Node/Python:**

- **Single static binary**: `cargo build --target x86_64-unknown-linux-musl --release` produces one file, no runtime dependencies
- **Memory safety without GC**: Critical for a long-running daemon that manages P2P connections, embedding inference, and in-memory indices
- **Ecosystem fit**: `rust-libp2p` is the most mature libp2p implementation. `ort` (ONNX Runtime) and `candle` (Hugging Face) are production-grade Rust ML crates. `instant-distance` provides a battle-tested HNSW implementation.
- **Performance**: Sub-millisecond embedding lookups, thousands of concurrent P2P connections on a single core
- **Kite alignment**: The existing Kite server and CLI are Rust. Shared crates, shared tooling, shared expertise.

**Docker Image Strategy:**

```dockerfile
# Stage 1: Build
FROM rust:1.78-alpine AS builder
RUN apk add --no-cache musl-dev pkgconfig openssl-dev
WORKDIR /build
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# Stage 2: Runtime
FROM scratch
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/kite-meshd /
COPY --from=builder /build/models/all-MiniLM-L6-v2-q8.onnx /models/
EXPOSE 4190 4191
ENTRYPOINT ["/kite-meshd"]
```

**Target image size**: <30MB (binary ~15MB + quantized model ~15MB)

### One-Click Deploy Templates

#### Docker Compose (self-host)

```yaml
version: "3.8"
services:
  kite-meshd:
    image: ghcr.io/kitemesh/kite-meshd:latest
    ports:
      - "4190:4190"   # P2P traffic
      - "4191:4191"   # Local API
      - "9191:9191"   # Metrics
    environment:
      - MESH_LISTEN_ADDR=/ip4/0.0.0.0/tcp/4190
      - MESH_API_ADDR=0.0.0.0:4191
      - MESH_TIER=local          # local | trust | global
      - MESH_AGENT_NAME=my-agent
      # Optional: Trust-Mesh PSK
      # - MESH_PSK=base64-encoded-psk
      # Optional: Global Swarm bootstrap
      # - MESH_BOOTSTRAP_PEERS=/ip4/x.x.x.x/tcp/4190/p2p/QmXYZ...
    volumes:
      - meshdata:/data
volumes:
  meshdata:
```

#### Fly.io (one command)

```bash
flyctl launch --from ghcr.io/kitemesh/kite-meshd:latest \
  --name my-mesh-node \
  --region ord \
  --vm-size shared-cpu-1x \
  --env MESH_TIER=trust
```

#### Railway (template)

One-click deploy button in the README. Railway template includes:
- Pre-configured Dockerfile
- Environment variable UI
- Automatic HTTPS for the API endpoint
- Volume mount for persistent state

#### Shuttle (Rust-native)

```rust
#[shuttle_runtime::main]
async fn main() -> shuttle_axum::ShuttleAxum {
    let mesh = kite_mesh_daemon::start(Config::from_env()).await?;
    Ok(mesh.api_router().into())
}
```

### Configuration Reference

```toml
# kite-meshd.toml

[identity]
name = "my-agent"                          # Human-readable name
# keypair auto-generated on first run, stored in data_dir

[network]
listen_addr = "/ip4/0.0.0.0/tcp/4190"     # P2P listen address
api_addr = "127.0.0.1:4191"               # Local API (bind to localhost!)
tiers = ["local"]                          # Which tiers to enable: local, trust, global

[network.local]
mdns_interval_secs = 30                    # mDNS discovery interval

[network.trust]
psk = ""                                   # Base64-encoded Pre-Shared Key
relay_servers = []                         # Circuit relay servers for NAT traversal

[network.global]
bootstrap_peers = [                        # Public DHT bootstrap nodes
  "/dns4/bootstrap1.kitemesh.dev/tcp/4190/p2p/QmXYZ...",
]

[routing]
embedding_model = "all-MiniLM-L6-v2-q8"   # Quantized model name
similarity_threshold = 0.85                # Minimum cosine similarity for match
gossip_timeout_ms = 200                    # Wait for gossip matches before DHT fallback
max_hnsw_entries = 10000                   # Max capability vectors in memory

[storage]
data_dir = "/data"                         # Persistent state directory
backend = "sqlite"                         # sqlite | postgres
# postgres_url = ""                        # Optional: Postgres connection string

[metrics]
enabled = true
addr = "0.0.0.0:9191"
```

---

## Part 4 — The Open-Source Client Library (kite-mesh-sdk)

### Purpose

The SDK is a Rust library that any agent framework can use to join the mesh. It abstracts away libp2p, embedding, and protocol details into a clean API.

### Repository Structure

```
kite-mesh-sdk/                      # Standalone repo: github.com/kitemesh/kite-mesh-sdk
├── Cargo.toml
├── LICENSE                         # Apache-2.0 OR MIT
├── README.md
├── src/
│   ├── lib.rs                      # Public API surface
│   ├── client.rs                   # MeshClient — the main entry point
│   ├── capability.rs               # Capability card builder
│   ├── intent.rs                   # Intent builder and broadcaster
│   ├── channel.rs                  # State channel abstraction
│   ├── trust.rs                    # Trust group management
│   ├── config.rs                   # Configuration
│   └── error.rs                    # Error types
├── examples/
│   ├── simple_agent.rs             # Minimal agent example
│   ├── python_executor.rs          # Agent that executes Python
│   └── web_scraper.rs              # Agent that scrapes URLs
└── tests/
```

### API Design

```rust
use kite_mesh_sdk::{MeshClient, CapabilityCard, Intent, Tool};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create a mesh client
    let client = MeshClient::builder()
        .name("my-python-agent")
        .tier_local()               // Enable mDNS discovery
        .tier_trust("base64-psk")   // Enable trust-mesh
        .build()
        .await?;
    
    // Publish capabilities
    client.publish_capability(
        CapabilityCard::builder()
            .tool(Tool::new("python_exec")
                .description("Execute Python 3.12 in sandboxed Docker container")
                .input_schema(include_str!("schemas/python_input.json"))
            )
            .tool(Tool::new("pip_install")
                .description("Install Python packages in isolated environment")
            )
            .ontology("code-execution")
            .ontology("python")
            .pricing_free()
            .build()?
    ).await?;
    
    // Listen for matching intents
    let mut intents = client.subscribe_intents().await?;
    
    while let Some(intent) = intents.next().await {
        println!("Received intent: {}", intent.description());
        
        // Accept and open state channel
        let channel = client.accept(intent).await?;
        
        // Stream results back
        channel.send_status("Starting execution...").await?;
        let result = execute_python(&intent.payload()).await?;
        channel.send_result(result).await?;
        channel.close().await?;
    }
    
    Ok(())
}
```

### Intent Broadcasting (Requester Side)

```rust
use kite_mesh_sdk::{MeshClient, Intent};

let client = MeshClient::builder()
    .name("coordinator")
    .tier_local()
    .build()
    .await?;

// Broadcast an intent and wait for a match
let matched = client.broadcast_intent(
    Intent::builder()
        .description("Execute Python script in sandboxed Linux environment with GPU")
        .ontology("code-execution")
        .payload(serde_json::json!({
            "script": "import torch; print(torch.cuda.is_available())",
            "timeout_secs": 30,
        }))
        .build()?
).await?;

// Open state channel with matched agent
let channel = matched.open_channel().await?;

// Stream logs
let mut logs = channel.subscribe_status();
while let Some(log) = logs.next().await {
    println!("[worker] {}", log);
}

// Get final result
let result = channel.await_result().await?;
println!("Result: {:?}", result);
```

### FFI Bindings (Future)

The SDK will provide C-compatible FFI bindings, enabling wrappers in:

- **Python**: `kite-mesh-py` (PyO3) — for Python agent frameworks (CrewAI, AutoGen, LangGraph)
- **TypeScript/Node**: `kite-mesh-js` (napi-rs) — for Node.js agent frameworks
- **Go**: `kite-mesh-go` (cgo) — for Go-based agents

This is Phase 2+ work. Phase 1 focuses on the Rust SDK and the sidecar daemon's HTTP API (which any language can call).

---

## Architecture Deep Dive

### Message Flow: Complete Intent Lifecycle

```
┌─────────────┐                    ┌─────────────┐
│ Coordinator │                    │   Worker    │
│   Agent     │                    │   Agent     │
└──────┬──────┘                    └──────┬──────┘
       │                                  │
       │  1. broadcast_intent()           │
       │  "Need Python sandboxed exec"    │
       ├──────────────────┐               │
       │                  ▼               │
       │         ┌────────────────┐       │
       │         │ Local Daemon   │       │
       │         │                │       │
       │         │ embed(intent)  │       │
       │         │ → [0.23, 0.87, │       │
       │         │    ..., 0.45]  │       │
       │         │                │       │
       │         │ HNSW lookup:   │       │
       │         │ found match!   │──────▶│  2. Intent metadata arrives
       │         │ sim = 0.94     │       │     via GossipSub
       │         └────────────────┘       │
       │                                  │
       │                                  │  3. Worker daemon checks
       │                                  │     local HNSW: match!
       │                                  │
       │  4. PROPOSE: "I can do this,     │
       │     here's my capability card"   │
       │◀─────────────────────────────────┤
       │                                  │
       │  5. AGREE: "Accepted, open       │
       │     state channel"               │
       ├─────────────────────────────────▶│
       │                                  │
       │  ═══ State Channel Established ═══
       │  (dedicated libp2p stream)       │
       │                                  │
       │  6. STREAM: status updates       │
       │◀─────────────────────────────────┤
       │  "Installing dependencies..."    │
       │◀─────────────────────────────────┤
       │  "Executing script..."           │
       │◀─────────────────────────────────┤
       │                                  │
       │  7. INFORM: final result         │
       │◀─────────────────────────────────┤
       │                                  │
       │  ═══ State Channel Closed ═══════
       │                                  │
```

### Security Model

#### Identity

Every agent has an Ed25519 keypair generated on first startup. The public key serves as the agent's permanent identity on the mesh. The private key never leaves the local machine.

Key rotation: Agents can rotate keys by publishing a signed "key rotation" message signed by both old and new keys.

#### Message Authentication

Every ACL message is signed by the sender's Ed25519 private key. Receivers verify signatures before processing. Unsigned or incorrectly signed messages are dropped silently.

#### Transport Encryption

All libp2p connections use Noise protocol for transport encryption (built into libp2p). Trust-Mesh connections add an additional PSK layer.

#### Capability Card Integrity

Capability cards are signed by the publishing agent. Endorsements are additional signatures from trusted peers. The chain of trust is verifiable without contacting the endorser.

#### Threat Model

| Threat | Mitigation |
|---|---|
| Sybil attacks (fake peers) | Proof-of-work on Global Swarm registration; reputation system; Trust-Mesh is inherently Sybil-resistant (PSK) |
| Capability spoofing | Signed capability cards; endorsement chains; reputation scores derived from verified task completions |
| Man-in-the-middle | Noise transport encryption; Ed25519 identity verification; PSK layer on Trust-Mesh |
| Denial of service (GossipSub flood) | Rate limiting per peer; message size limits; GossipSub's built-in flood protection |
| Data exfiltration via state channels | State channels are end-to-end encrypted; payload schemas validated before processing |
| Malicious agent execution | Out of scope for the mesh — the mesh routes, the host agent decides whether to execute. Sandbox isolation is the host agent's responsibility. |

### A2A Protocol Compatibility

Google's Agent2Agent (A2A) Protocol (v0.3.0+, now under Linux Foundation governance) is the emerging standard for agent interoperability. The Kite Agent Mesh should implement an A2A compatibility layer:

**Mapping:**

| A2A Concept | Kite Mesh Equivalent |
|---|---|
| Agent Card | Capability Card |
| Task | Intent → Match → State Channel lifecycle |
| Message (with Parts) | ACL Message payload |
| Artifact | ResultPayload |
| Push Notifications (SSE) | GossipSub + State Channel streams |

**Implementation**: An `a2a-bridge` crate that translates between A2A JSON-RPC messages and Kite Mesh protobuf ACL messages. This allows any A2A-compatible agent to discover and interact with mesh agents, and vice versa.

This is a Phase 2 priority. Phase 1 focuses on the native mesh protocol. A2A compatibility ensures the mesh doesn't become another walled garden.

---

## Competitive Landscape and Differentiation

### How We're Different

| Feature | Kite Mesh | A2A (Google) | CrewAI | AutoGen | LangGraph |
|---|---|---|---|---|---|
| **Transport** | libp2p (P2P, no server) | HTTPS (server-dependent) | In-process | In-process/async | In-process |
| **Discovery** | Semantic (vector DHT) | Static Agent Cards (URL-based) | Code-defined | Code-defined | Code-defined |
| **Network** | Decentralized mesh | Client-server | Single process | Single process | Single process |
| **Trust** | Cryptographic (Ed25519 + PSK) | OAuth 2.0 / API keys | N/A (same process) | N/A | N/A |
| **Local-first** | Yes (mDNS, zero internet) | No (requires HTTP) | Yes (in-process) | Yes (in-process) | Yes (in-process) |
| **Cross-framework** | Yes (any language via SDK/sidecar) | Yes (HTTP is universal) | No (Python-only) | No (Python-only) | No (Python-only) |
| **Payment rails** | Built-in (x402) | None | None | None | None |
| **Semantic routing** | Built-in (embedding-based) | Manual matching | Manual assignment | Manual routing | Graph-defined |

### Key Differentiators

1. **Decentralized by default**: No central server required. Agents discover each other directly. This is architecturally unique in the agent orchestration space.

2. **Semantic intent matching**: Agents don't need to know each other's names or addresses. They describe what they need, and the network finds the best match mathematically.

3. **Three-tier topology**: From zero-config local (for dev) to permissioned trust networks (for teams) to global public (for marketplaces) — all with the same protocol.

4. **Webhook bridge**: No other agent mesh integrates with external event sources (GitHub, Stripe, etc.) natively. Kite's existing webhook infrastructure is a unique competitive advantage.

5. **Payment integration**: The Global Swarm tier has built-in payment rails via x402. Agents can charge for services without external payment infrastructure.

---

## Phasing and Milestones

### Phase 1: Local Dev-Swarm (MVP) — 8-10 weeks

**Goal**: Two agents on the same machine discover each other via mDNS, match capabilities semantically, establish a state channel, execute a task, and stream results.

**Deliverables:**

- `kite-mesh-core` crate: ACL types, Capability Cards, Identity management
- `kite-mesh-proto` crate: Protobuf definitions, generated code
- `kite-mesh-transport` crate: libp2p with mDNS + GossipSub (Tier 1 only)
- `kite-mesh-routing` crate: ONNX embedding engine + HNSW index
- `kite-mesh-daemon` binary: Sidecar with local HTTP API
- `kite-mesh-sdk` crate: Rust client library with builder API
- Docker image: `ghcr.io/kitemesh/kite-meshd:0.1.0`
- Examples: `local_swarm` (two agents collaborating)
- Kite integration: `kite mesh start`, `kite mesh status`, `kite mesh peers`

**Definition of Done:**

```bash
# Terminal 1: Start a "Python executor" agent
kite-meshd --name python-worker --capability "Execute Python in Docker sandbox"

# Terminal 2: Start a "Coordinator" agent
kite-meshd --name coordinator

# Terminal 2: Broadcast an intent
curl http://localhost:4191/intent \
  -d '{"description": "Run this Python script in a sandbox", "payload": {"script": "print(42)"}}'

# Expected: python-worker matches (cosine sim > 0.85), state channel opens, result streams back
# Response: {"matched_agent": "python-worker", "result": "42", "latency_ms": 23}
```

### Phase 2: Trust-Mesh + Kite Integration — 8-10 weeks

**Goal**: Agents on different machines collaborate via PSK-protected connections. Kite dashboard shows fleet status and mesh topology.

**Deliverables:**

- Trust-Mesh (Tier 2): PSK auth, circuit relays, Kademlia scoped DHT
- Kite CLI: `kite mesh trust keygen/add/revoke/list`
- Kite dashboard: Fleet Overview page, Mesh Topology visualization
- Webhook → Mesh bridge: `kite stream --sink mesh`
- A2A compatibility layer (bridge crate)
- One-click deploy templates: Fly.io, Railway

### Phase 3: Global Swarm + Marketplace — 10-12 weeks

**Goal**: Public DHT, reputation system, payment integration, FFI bindings for Python/Node/Go.

**Deliverables:**

- Global Swarm (Tier 3): Public Kademlia DHT, bootstrap nodes, reputation scoring
- x402 payment integration for Global Swarm tasks
- Reputation system: Task completion receipts, score aggregation
- Spam prevention: Proof-of-work on capability registration
- FFI bindings: Python (PyO3), Node (napi-rs)
- Kite dashboard: Marketplace view, cost tracking
- Enterprise features: Custom trust policies, audit logs, SSO

### Phase 4: Ecosystem Growth — Ongoing

- Community bootstrap node program
- Agent framework integrations (CrewAI, AutoGen, LangGraph plugins)
- Mobile agent support (Candle WASM for browser-based agents)
- Edge deployment (ARM64 builds for Raspberry Pi, Jetson)
- Agent marketplace (searchable directory of public agents)

---

## Open Questions and Risks

### Technical Risks

| Risk | Severity | Mitigation |
|---|---|---|
| **Embedding model size** (~22MB) bloats the daemon for resource-constrained environments | Medium | Offer "headless" mode that accepts pre-computed embeddings via API; explore smaller models (e.g., E5-small at 17MB) |
| **ONNX Runtime startup latency** on cold boot (~500ms) | Low | Pre-warm on daemon start; cache compiled model; alternative: Candle with ahead-of-time compilation |
| **libp2p complexity** — large dependency tree, steep learning curve | Medium | Abstract behind clean transport trait boundary; extensive integration tests; leverage community examples |
| **NAT traversal reliability** — circuit relays can be unreliable | Medium | Provide hosted relay nodes for paid Kite tiers; fallback to TURN-like WebRTC relay; QUIC for UDP-based hole punching |
| **Semantic matching quality** — small models may not distinguish nuanced capabilities | Medium | Configurable threshold; hybrid approach (semantic + keyword matching); allow manual capability tagging as fallback |

### Product Risks

| Risk | Severity | Mitigation |
|---|---|---|
| **Adoption** — developers may not see value vs. simple HTTP APIs | High | Focus Phase 1 on local dev experience where the value is immediate (zero-config discovery). Build compelling demos. |
| **Open-source competition** — someone forks and builds a better client | Low | Kite's advantage is the full stack: webhooks + mesh + dashboard + billing. The open-source mesh brings users to the ecosystem. |
| **A2A Protocol wins** and becomes the standard, making our protocol irrelevant | Medium | A2A compatibility layer ensures we interoperate. Our protocol adds P2P and semantic routing — features A2A doesn't have. |
| **Kite brand confusion** — "is Kite a webhook tool or an agent mesh?" | Medium | Clear naming: "Kite" is the product, "Kite Mesh" is the protocol. The mesh repo is `kitemesh/kite-mesh`, not `kite/mesh`. |

### Open Design Questions

1. **Embedding model selection**: `all-MiniLM-L6-v2` (384-dim, 22MB) vs. `E5-small` (384-dim, 17MB) vs. `BGE-micro` (256-dim, 14MB). Trade-off between match quality and size. Need benchmarks on real capability card / intent pairs.

2. **GossipSub vs. custom routing**: GossipSub is general-purpose pub/sub. For semantic routing, we might want a custom protocol that carries embedding vectors in the gossip metadata. Need to evaluate overhead.

3. **State Channel persistence**: Should state channels persist across daemon restarts? Argument for: long-running tasks survive restarts. Argument against: complexity, state corruption risk. Current lean: ephemeral channels with task checkpointing.

4. **Global Swarm governance**: Who runs bootstrap nodes? How do we prevent centralization? Options: community-operated (like Bitcoin nodes), foundation-operated (like libp2p bootstrap), or hybrid.

5. **Capability Card versioning**: How do we handle breaking changes to the capability card schema? Protobuf gives us forward/backward compatibility for field additions, but semantic changes (redefining what "code-execution" ontology means) need a versioning strategy.

6. **Multi-model embeddings**: If different nodes use different embedding models, vectors aren't directly comparable. Options: mandate a single model, provide a translation layer, or use model-agnostic distance metrics.

---

## Appendices

### Appendix A: Technology Stack Summary

| Component | Technology | Rationale |
|---|---|---|
| Language | Rust | Performance, safety, single binary, ecosystem (libp2p, ort, candle) |
| P2P Networking | rust-libp2p | Most mature libp2p implementation, production-proven (IPFS, Polkadot) |
| Pub/Sub | GossipSub (libp2p) | Efficient gossip-based pub/sub with flood protection |
| Local Discovery | mDNS (libp2p) | Zero-config LAN discovery, no internet required |
| Internet Discovery | Kademlia DHT (libp2p) | Proven distributed hash table for internet-scale peer lookup |
| NAT Traversal | Circuit Relay + QUIC (libp2p) | Handles firewalls, NATs, and restricted networks |
| Serialization | Protocol Buffers (prost) | Strict typing, compact wire format, excellent Rust tooling |
| Embedding Inference | ONNX Runtime (ort) or Candle | Local, fast, no external API dependency |
| Vector Index | HNSW (instant-distance or hora) | Sub-millisecond approximate nearest neighbor search |
| Embedding Model | all-MiniLM-L6-v2 (INT8) | 384-dim, 22MB, <5ms inference, well-benchmarked |
| State Store | SQLite (rusqlite) | Zero-config, embedded, sufficient for single-node state |
| Metrics | prometheus-client | Consistent with existing Kite observability stack |
| Build/CI | GitHub Actions | Consistent with existing Kite CI/CD |
| Container | Docker (multi-stage, scratch base) | <30MB final image, runs anywhere |

### Appendix B: Dependency Sizing Estimates

| Crate | Approximate Binary Contribution | Notes |
|---|---|---|
| rust-libp2p (core + protocols) | ~3-5 MB | Selective feature flags minimize bloat |
| ort (ONNX Runtime) | ~5-8 MB | Static linking of ONNX Runtime C++ core |
| prost (Protobuf) | ~200 KB | Code generation, minimal runtime |
| instant-distance (HNSW) | ~100 KB | Pure Rust, minimal |
| axum (HTTP API) | ~1 MB | Shared with hyper/tokio |
| tokio (async runtime) | ~1-2 MB | Already required by libp2p |
| rusqlite (SQLite) | ~1 MB | Bundled SQLite |
| **Total binary estimate** | **~12-18 MB** | Before strip + UPX compression |

### Appendix C: Kite Codebase Integration Points

| Existing Kite Module | Mesh Integration |
|---|---|
| `crates/kite-server/src/federation/` | Federation logic ports to mesh transport (outbox worker, delivery tracking, retry) |
| `crates/kite-server/src/broadcast.rs` | WebSocket broadcast evolves to include mesh-connected agents as recipients |
| `crates/kite-cli/src/sinks/` | New `MeshSink` added alongside existing 8 sink types |
| `crates/kite-cli/src/commands/` | New `mesh` command group (start, stop, status, cap, intent, trust, peers, topology) |
| `crates/kite-protocol/src/` | ACL message types extend CloudEvents extensions |
| `apps/web/src/app/dashboard/` | New pages: Fleet, Topology, Capabilities, Cost Center |
| `infra/prod/prometheus/` | New mesh-specific recording rules and dashboards |
| `crates/kite-server/src/x402/` | Payment rails exposed for Global Swarm task billing |
| `crates/kite-cli/src/queue.rs` | SQLite DLQ pattern reused for offline mesh message queuing |
| `crates/kite-server/src/routes/skills.rs` | Skill registry maps to capability card publishing |

### Appendix D: Glossary

| Term | Definition |
|---|---|
| **ACL** | Agent Communication Language — the structured message format for all mesh communication |
| **Capability Card** | A signed, structured description of what an agent can do, broadcast to the mesh for discovery |
| **Cosine Similarity** | A measure of similarity between two vectors, ranging from -1 to 1. Used to match intents to capabilities. |
| **GossipSub** | A pub/sub protocol where peers exchange metadata about messages and only request full messages if interested |
| **HNSW** | Hierarchical Navigable Small World — an algorithm for fast approximate nearest-neighbor search in high-dimensional spaces |
| **Intent** | A structured broadcast describing what an agent needs, routed semantically to capable peers |
| **Kademlia** | A distributed hash table protocol used for peer and data discovery across the internet |
| **mDNS** | Multicast DNS — zero-configuration peer discovery on local networks |
| **Performative** | The type of speech act in an ACL message (request, propose, agree, inform, etc.) |
| **PSK** | Pre-Shared Key — a symmetric key shared between trusted peers for authenticated connections |
| **Sidecar** | A daemon process that runs alongside an agent, handling mesh networking on its behalf |
| **State Channel** | A dedicated, persistent, bi-directional stream between two matched agents |
| **Vector DHT** | A Distributed Hash Table that routes by semantic similarity (embedding distance) rather than key hash |
