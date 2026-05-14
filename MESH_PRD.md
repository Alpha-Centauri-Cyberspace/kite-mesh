# Kite Agent Mesh — Product Requirements Document

> This is the **north-star PRD** for the full Kite Mesh platform. For the first-phase build plan that we're executing on right now, see [`docs/walking_skeleton.md`](./docs/walking_skeleton.md).
>
> *Last updated: 2026-04-23.*

> **Status**: Draft
> **Scope**: Open-source mesh protocol, local daemon, SDK, and Kite commercial integration

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Product Thesis, Goals, and Non-Goals](#product-thesis-goals-and-non-goals)
3. [Relationship to the Agent-Interop Landscape](#relationship-to-the-agent-interop-landscape)
4. [System Overview](#system-overview)
5. [Core Architecture](#core-architecture)
6. [Protocol and Data Model](#protocol-and-data-model)
7. [Discovery Algorithms and Provable Claims](#discovery-algorithms-and-provable-claims)
8. [Local Runtime, Transport, and Security](#local-runtime-transport-and-security)
9. [Kite Product Integration](#kite-product-integration)
10. [Local Daemon](#local-daemon)
11. [SDK Design](#sdk-design)
12. [Phasing and Milestones](#phasing-and-milestones) (incl. Phase 00 — Walking Skeleton)
13. [Risks and Open Research Questions](#risks-and-open-research-questions)
14. [Benchmark and Validation Plan](#benchmark-and-validation-plan)
15. [Appendix A — Repository Structure](#appendix-a--repository-structure)
16. [Appendix B — Configuration Sketch](#appendix-b--configuration-sketch)
17. [Appendix C — Mathematical Notes](#appendix-c--mathematical-notes)
18. [Appendix D — References](#appendix-d--references)

---

## Executive Summary

**Kite is the substrate layer for agent collaboration — the plumbing beneath MCP's tool access and A2A's federated handoff.** Where MCP connects an agent to its tools and A2A connects servers across administrative boundaries, Kite provides what neither specifies: local-first peer discovery, cryptographic identity, semantic capability matching, and explicit negotiation, for agents that run on laptops, homes, regional clusters, and air-gapped trust meshes as naturally as on the public internet.

The framing is **TCP/IP for agent collaboration**: a protocol simple enough to run peer-to-peer on a laptop, strong enough to support cross-organizational trust, boring enough that other protocols layer on top without asking permission. MCP and A2A are applications that can run over Kite; Kite is not an application that competes with them.

```text
┌─────────────────────────────────────────────────────────────┐
│     Agent frameworks (LangChain, CrewAI, ADK, OpenAI SDK)   │
├─────────────────────────────────────────────────────────────┤
│  Application protocols:  MCP  │  A2A  │  AGNTCY  │  ANP     │
├─────────────────────────────────────────────────────────────┤
│  KITE MESH — discovery · identity · negotiation · trust     │
│  (facet-first cards · angular LSH · FIPA ACL · local tiers) │
├─────────────────────────────────────────────────────────────┤
│     libp2p substrate — Kademlia · GossipSub · Noise · QUIC  │
└─────────────────────────────────────────────────────────────┘
```

The design rests on two load-bearing decisions:

**(1) Layering.** Kite is a substrate, not an application. Its job is to get two opaque agents talking to each other across trust boundaries. What they say to each other — MCP, A2A, custom ACL payloads — is not Kite's problem. This keeps the mesh thin, versionable, and neutral across the application protocols that already exist.

**(2) Metric discipline.** Kademlia handles exact key-based routing. Semantic discovery is a directory problem, not a routing-metric problem. Raw-embedding greedy routing is mathematically unsound in high dimensions [10][11]; Kite refuses to claim what cannot be proved.

From these two decisions, five concrete commitments follow:

1. **Facet-first capability modeling.** Every capability is split into *exact facets* (tools, ontologies, resource class, pricing band, trust tier, region, status) and a *soft semantic description*. Exact requirements are not approximated by embeddings when they can be encoded structurally.

2. **Angular LSH directory over exact DHT keys.** Capabilities are indexed by exact DHT keys derived from canonical facet fingerprints, model identifiers, and angular LSH codes over the embedding. Kademlia's proven O(log n) lookup behavior [1][9] is preserved; semantic similarity is expressed through collisions in a random-hyperplane LSH family with a closed-form recall bound [2][7].

3. **Hash-prefix GossipSub topics.** Coarse semantic topics are constructed from LSH prefixes. Standard GossipSub behavior runs unmodified inside each topic, inheriting the formal resilience analysis of Kumar et al. (IEEE S&P 2024) [4]. There is no content-based filtering *inside* a topic mesh, where filtering would break message-complexity guarantees.

4. **Local reranking after candidate retrieval.** Candidate sets are built from exact filters plus LSH collisions, then reranked locally with exact cosine similarity, facet checks, and policy. Global semantic ordering is not claimed; bounded candidate recall plus exact local rerank is.

5. **One canonical encoder per production epoch.** Cross-model embedding comparability is a research problem; cosine similarity is meaningful only across vectors from the same geometry. Multi-model interoperability is deferred to the research track with alignment certificates.

Local ANN (HNSW) is an optional local optimization [3], not part of the global correctness story. Distributed HNSW consistency is out of scope. Greedy routing in raw embedding space is out of scope — high-dimensional geometry makes it unsound [10].

This gives Kite a product that is decentralized, local-first, commercially expandable, and mathematically grounded in the regimes where each component's proofs actually apply.

---

## Product Thesis, Goals, and Non-Goals

### Thesis

Kite is **TCP/IP for agent collaboration**: a substrate for agent-to-agent peer discovery, identity, and negotiation, beneath the application protocols (MCP, A2A, AGNTCY) that specify what agents *say* once they find each other. The substrate is built from components that each stay inside their valid mathematical regime.

Three properties define the substrate posture:

- **Neutrality.** The mesh does not pick winners among application protocols. A Kite capability card can advertise MCP servers, an A2A Agent Card URL, or neither. Bridges are first-class, not afterthoughts.
- **Local-first addressability.** An agent on a laptop with no public IP is a first-class mesh participant. The public swarm is a deployment mode, not a prerequisite.
- **Thin and versionable.** The protocol specifies discovery, identity, and negotiation envelopes. It does not specify task semantics, payment semantics, or tool semantics — those are application concerns.

### Primary goals

1. **Local-first by default.** Two agents on the same machine or LAN should discover and collaborate with zero central coordinator.
2. **Semantic discovery without semantic routing claims.** Discovery feels intent-based to users; the implementation stays grounded in exact key lookup and bounded candidate generation.
3. **Composable trust.** Identity, signed capability publication, PSK trust domains, endorsements, and optional public reputation coexist.
4. **Clean operational fit for Kite.** Kite is the easiest operational layer for running the mesh in real teams and real event-driven workflows.
5. **A research runway.** Production leaves room for novel follow-on work (projected metric overlays, alignment certificates) without coupling it to shipping.

### Non-goals for the MVP

1. Greedy routing in raw embedding space.
2. Heterogeneous embedding models in production discovery.
3. Globally consistent distributed HNSW.
4. One universal similarity threshold across all domains.
5. Large capability cards stored directly as heavy DHT values.
6. A public swarm as the critical path for early product value.

---

## Relationship to the Agent-Interop Landscape

The 2025–26 landscape produced several agent-interop protocols with overlapping scope. Kite is designed to **coexist with and compose across** them, not to replace any of them. Positioning matters because the landscape is crowded and the wrong framing wastes reviewer attention.

### MCP — vertical agent↔tool protocol

Anthropic's Model Context Protocol — governed by the Linux Foundation's Agentic AI Foundation (AAIF) since December 2025, adopted by OpenAI, Google, Microsoft, and AWS — standardizes the **vertical** axis: how a single agent (or host) reaches its tools, resources, and prompts through a host↔client↔server topology over stdio or Streamable HTTP [29]. MCP has no primitive for peer discovery, no decentralized trust model beyond OAuth to a known server, and an official-registry-as-root-of-trust assumption that Kite explicitly rejects in favor of libp2p Kademlia, angular LSH, and local-first tiering.

Kite operates on the orthogonal **horizontal** axis — how opaque peer agents discover each other, match capabilities semantically, and negotiate handoffs without a central registry or shared host. Google's A2A team captures the framing cleanly: *"A2A focuses on agents partnering on tasks, whereas MCP focuses on agents using capabilities"* [33]. The two protocols compose: a Kite capability card may advertise an `mcp_servers` facet, so peers discovered through the mesh can still be consumed as MCP endpoints by any MCP-aware host. Prior art for this pattern exists in SEP-2127 (MCP Server Cards) [32] and in AGNTCY's OASF extensions [31], which embed MCP server descriptors as first-class directory facets.

### A2A — federated agent↔agent protocol

Google's Agent2Agent protocol (launched April 2025, donated to the Linux Foundation June 2025, absorbed IBM's ACP August 2025) standardizes **federated** agent-to-agent collaboration: each agent publishes a JSON Agent Card at `/.well-known/agent-card.json`, speaks JSON-RPC 2.0 over HTTPS with SSE streaming, and exposes a Task lifecycle (`submitted → working → input-required → completed/failed/canceled/rejected`) to partners who can reach its public endpoint [30]. A2A assumes every participating agent is a reachable HTTPS endpoint owned by an identifiable enterprise. This works well at the federation boundary between administrative domains. It does not work when agents run inside homes, laptops, regional clusters, or air-gapped trust meshes — the local-first case that motivates Kite.

Kite operates one layer below A2A's assumptions. The libp2p Kademlia DHT gives O(log n) exact-key routing without DNS or well-known URLs; angular LSH over canonical embeddings gives semantic candidate generation without a central index; GossipSub gives push-style capability announcements; Noise + Ed25519 gives peer identity without a CA. The three-tier model (mDNS subnet → PSK trust mesh → public swarm) admits deployments where A2A's HTTPS-endpoint assumption is infeasible. The FIPA ACL performatives (REQUEST, PROPOSE, AGREE, REFUSE, INFORM, FAILURE, CANCEL, SUBSCRIBE) formalize the negotiation vocabulary that A2A's task lifecycle only implies — a principled revival, not legacy baggage: no other 2025–26 protocol cites FIPA directly, leaving a defensible differentiator for a spec that wants explicit speech-act semantics.

### AGNTCY — the closest published cousin

Cisco's AGNTCY Agent Directory Service (IETF `draft-mp-agntcy-ads`, arXiv 2509.18787) [31] is architecturally the closest published system to Kite. Its OASF record extensions include MCP-server and A2A-handshake metadata as first-class facets, and the directory is built on a **Kademlia-based DHT with a two-level capability → content mapping** — a direct validation of Kite's core architectural choice. Kite and AGNTCY ADS share the Kademlia-backed capability index; they differ in four meaningful ways:

1. **Local-first tiering** (mDNS subnet / PSK trust mesh / public swarm). AGNTCY assumes global addressability from the outset.
2. **Angular LSH on the data plane**, not just a directory index. Kite's semantic candidate generation uses LSH collisions with a closed-form recall bound; AGNTCY uses exact capability keys only.
3. **FIPA ACL performatives as a first-class negotiation layer.** AGNTCY has no native negotiation primitives.
4. **Single composable libp2p substrate for discovery and messaging.** AGNTCY layers gRPC/SLIM messaging on top of a separate DHT directory.

### Landscape summary

| Protocol | Primary layer | Topology | Transport | Peer discovery | Identity | Negotiation primitive |
|---|---|---|---|---|---|---|
| MCP [29] | Agent ↔ tools | Host–client–server (star) | stdio / Streamable HTTP | None (registry lookup) | OAuth | None |
| A2A [30] | Agent ↔ agent | Federated HTTPS | JSON-RPC 2.0 / gRPC / REST | `.well-known/agent-card.json` | OpenAPI auth | Task lifecycle (implicit) |
| ANP | Agent ↔ agent | Decentralized | HTTP + JSON-LD | DID + RFC 8615 | W3C DIDs + VCs | Meta-protocol negotiation |
| AGNTCY ADS [31] | Directory | Kademlia DHT (control) + SLIM (data) | gRPC + MLS | DHT capability index | DIDs + Agent Badges | None native |
| **Kite** | Agent ↔ agent discovery + negotiation | libp2p P2P, three-tier local-first | libp2p (TCP/QUIC) + Noise | Kademlia DHT + LSH + GossipSub | Ed25519 peer ID | FIPA ACL performatives |

### Composition strategy

Kite does not aim to displace A2A's server-to-server federation, MCP's tool-access protocol, or AGNTCY's identity/badge infrastructure. It aims to fill the local-first, peer-discoverable, mathematically-grounded layer that none of the other protocols currently occupies. Three composition patterns are explicit in the design:

- **Kite ↔ MCP.** Kite capability cards carry an optional `mcp_servers` facet. Peers discovered over Kite can be consumed as MCP endpoints by any MCP-aware host.
- **Kite ↔ A2A.** A Kite capability card is a strict superset of the A2A Agent Card's discovery surface. A lightweight gateway (Phase 4) exposes any Kite peer as an A2A-reachable agent and any A2A agent as a discoverable Kite peer, translating capability card → Agent Card, GossipSub events → SSE streams, and ACL performatives → task lifecycle states.
- **Kite ↔ AGNTCY.** Kite directory records and OASF records can be bridged bidirectionally at the facet level, since both index capabilities over a Kademlia DHT.

Independent critiques that reinforce the layering argument — Simon Willison's "lethal trifecta" for multi-server MCP, the Trust Fabric survey's observation that *"[MCP, A2A, ACP] primarily target execution orchestration and fail to adequately address the deeper infrastructural needs of agent discovery, semantic identity, and dynamic trust management at scale"* [34], and Anthropic's own Jan 2026 "MCP Tool Search" admission that MCP's flat tool namespace does not scale past a single agent — all point to an under-served horizontal discovery layer. Kite targets exactly that layer.

---

## System Overview

The system is divided into three planes.

### 1. Control plane

Identity, trust, peer connectivity, transport setup, relays, and peer liveness.

Core technologies:
- Ed25519 identity [16]
- Noise transport security [17][18][19][20]
- mDNS for local discovery
- libp2p Kademlia for exact-key directory lookup [1]
- Circuit relay / NAT traversal for remote peers [21]

### 2. Discovery plane

This plane answers: *who can do this task?*

- Capability cards with exact facets plus embeddings
- Angular LSH signatures over normalized embeddings [2][7]
- Kademlia directory records keyed by `(facet_fingerprint, model_id, table_id, hash_code)`
- Hash-prefix GossipSub topics for push-style discovery [4]
- Local reranking against exact cosine, facet filters, and policy

### 3. Execution plane

This plane answers: *once we found a match, how do two agents work together?*

- ACL request / propose / agree / inform protocol
- Dedicated state channels after matching
- Result receipts, optional billing, reputation signals

### High-level message flow

```text
Intent text + exact requirements
        |
        v
Embed with canonical encoder
        |
        v
Compute facet fingerprint + LSH codes
        |
        +--> Push path: publish IntentAdvert to coarse hash-prefix topics
        |
        +--> Pull path: DHT lookup of exact directory keys
        |
        v
Union candidate records
        |
        v
Fetch full capability cards / verify signatures
        |
        v
Exact facet filter + cosine rerank + policy checks
        |
        v
ACL negotiation -> state channel -> execution -> receipt
```

---

## Core Architecture

### Facet-first capability model

A capability is not represented as "just an embedding." Each capability is split into two parts.

**Exact facets** — fields that behave like structured filters rather than soft semantics:

- `tools`: `python_exec`, `bash`, `web_scrape`, `browser_automation`
- `ontologies`: `code-execution`, `data-analysis`, `payments`, `github-events`
- `sandbox_level`: none / process / container / VM
- `network_access`: offline / egress-limited / full
- `gpu_class`: none / consumer / datacenter
- `privacy_tier`: public / trusted / confidential
- `region`: local / same-LAN / same-org / geographic tags
- `pricing_band`: free / low / medium / premium
- `trust_tier`: local / PSK / endorsed / public
- `status`: available / busy / draining / offline

These fields are canonicalized into a deterministic `facet_fingerprint`.

**Soft semantic description** — free text such as *"Executes Python 3.12 in a sandboxed Linux container with optional CUDA access and a 60-second timeout."* This text is embedded with the canonical encoder, L2-normalized, and used only for LSH code generation and local similarity reranking.

**Principle:** exact requirements should not be approximated by embeddings if they can be represented structurally.

### Canonical mesh encoder

The mesh uses one canonical embedding model per production epoch.

A capability card is semantically eligible for automatic matching only if:

- `model_id` is supported by the node
- the embedding dimension matches the mesh epoch
- the vector is L2-normalized and signed as part of the card

Cosine similarity is meaningful only when vectors come from the same embedding geometry. Sentence-Transformers models such as all-MiniLM-L6-v2 are trained with contrastive objectives that explicitly optimize cosine similarity between semantically related pairs and produce L2-normalized outputs by default [13]. Supporting heterogeneous models in the MVP would make the central similarity score ambiguous and brittle.

Cross-model discovery is deferred to the research track (Alignment Certificates), built around shared public anchor corpora, orthogonal or near-orthogonal alignment maps, and signed alignment certificates with explicit error budgets.

### Angular LSH directory

For each capability `c`, define:

- `F(c)` = canonical facet fingerprint
- `v(c)` = normalized embedding
- `h_i(v(c))` = the `i`-th angular LSH code for `v(c)` [2][7]

For each of `L` hash tables, publish a small signed directory record under the exact DHT key:

```text
K_i(c) = Hash(protocol_version || F(c) || model_id || i || h_i(v(c)))
```

The value stored in the DHT is a compact `DirectoryRecord`, not the full capability card.

**Why this works:**

- Kademlia continues to route exact keys with its standard logic — O(log n) lookups with expected length `0.173 × log₂(n)` hops for bucket size k = 20 [1][9]
- Semantics are expressed through collisions in an LSH family with a closed-form collision probability [2]
- Candidate recall is governed by known probability bounds ([Appendix C](#appendix-c--mathematical-notes))
- Capability cards can be refreshed, revoked, and re-published without changing the overlay routing metric

**Directory record contents:** `capability_id`, `agent_id`, `peer_id`, `facet_fingerprint`, `model_id`, `table_id`, `hash_code`, `status`, `ttl`, `signature`. The full card is fetched separately.

### Hash-prefix topics for push discovery

GossipSub is preserved by **not filtering messages inside a topic based on semantic opinions**. Each node subscribes to coarse topics derived from the same LSH family:

```text
mesh/<ontology>/<tool-class>/<table-id>/<prefix-l-bits>
```

Where `ontology` and `tool-class` come from exact facets, `table-id` is the LSH table index, and `prefix-l-bits` is a short prefix of the full LSH code.

**Publishing.** An intent produces a compact `IntentAdvert` containing: `intent_id`, `requester_peer_id`, exact facet slice, `model_id`, per-table LSH signatures, timeout, signature. The advert goes to the matching prefix topics.

**Receiving.** A peer receives the advert because it subscribed to that prefix topic. It locally decides whether to ignore, request the full intent, or respond with a proposal.

The topic mesh itself remains ordinary GossipSub: bounded-degree peer meshes per topic, eager-push to D mesh peers (default D = 6–8), IHAVE/IWANT gossip to the remainder. This yields O(D × N) total messages per broadcast with a small constant, and propagation latency on the order of log N / log D hops [4][15]. Formal resilience of the GossipSub scoring function is established for correctly-parameterized deployments; score-function misconfiguration can produce synthesizable attacks [4], so production parameters track the FileCoin-style configuration shown secure in that analysis.

**Content-based filtering inside a topic is explicitly avoided.** No formal analysis of semantic filtering on top of GossipSub exists; different nodes filtering differently would create uncharacterized risks to mesh delivery invariants. Selecting *topics* is a design knob; redefining forwarding correctness *inside* a topic is not.

### Query path

When an agent needs help, the discovery path is:

1. Canonicalize exact requirements into a facet slice
2. Embed the free-text description with the canonical encoder
3. Compute `L` LSH codes
4. Push path: publish `IntentAdvert` to coarse topics
5. Pull path (in parallel): Kademlia lookups for the exact directory keys
6. Union all candidate directory records
7. Fetch full capability cards where needed
8. Verify signatures and liveness
9. Apply exact facet filter
10. Rerank candidates by exact cosine similarity and policy
11. Apply per-slice thresholds
12. Open ACL negotiation with the top candidate(s)

### Matching policy

Each slice has two thresholds:

- `suggest_threshold(slice)` — lower bar for "show or consider this candidate"
- `accept_threshold(slice)` — higher bar for "auto-propose or auto-select this candidate"

Empirical evidence from sentence-transformer benchmarks supports per-slice calibration rather than a universal threshold: near-paraphrases score around 0.89 on the all-MiniLM-L6-v2 card, clearly-related-but-differently-worded pairs score ~0.67, and unrelated pairs sit between −0.05 and 0.15 [13]. The optimal threshold on the Quora Duplicate Questions benchmark with a closely-related model (all-mpnet-base-v2) is 0.8352 for accuracy and 0.7715 for F1 [25]. A comprehensive 2025 survey of similarity thresholds across models and tasks found optima ranging from 0.334 to 0.867 [24]. A single global threshold of 0.85 is defensible for near-duplicate matching but too aggressive for general discovery — per-slice calibration is required to serve code-execution, web-scraping, GPU inference, and billing tasks correctly.

### Local search policy

The local daemon keeps a cache of capability cards and performs local search before involving broader discovery.

- Up to `local_flat_scan_max` cards: exact flat scan over normalized vectors
- Above that threshold: optional HNSW for speed
- Flat scan remains available as a correctness fallback at all times

At modest local cardinalities, flat scan is fast enough and simpler to reason about. Brute-force search over 10,000 vectors of 384 dimensions requires ~10,000 dot products — well under 1 ms on any modern CPU. Weaviate's production defaults already use a flat index below 10,000 objects and only switch to HNSW above that threshold [22]. HNSW's O(log n) search [3] is valuable as the local index grows, but the product's correctness does not depend on it.

---

## Protocol and Data Model

### ACL envelope

The ACL is the mesh's common message envelope.

```protobuf
message AclMessage {
  string sender_id = 1;
  bytes signature = 2;
  uint64 timestamp_ns = 3;
  string message_id = 4;

  Performative performative = 5;
  string ontology = 6;
  string conversation_id = 7;
  string in_reply_to = 8;

  oneof payload {
    Intent intent = 10;
    IntentAdvert advert = 11;
    Proposal proposal = 12;
    Agreement agreement = 13;
    Result result = 14;
    StreamFrame stream = 15;
    CapabilityCard capability = 16;
    DirectoryRecord directory_record = 17;
    Receipt receipt = 18;
  }

  TrustContext trust = 20;
}
```

Performatives: `REQUEST`, `PROPOSE`, `AGREE`, `REFUSE`, `INFORM`, `FAILURE`, `CANCEL`, `SUBSCRIBE`.

### CapabilityCard

```protobuf
message CapabilityCard {
  string capability_id = 1;
  string agent_id = 2;
  string agent_name = 3;
  string agent_version = 4;

  repeated string ontologies = 5;
  repeated Tool tools = 6;

  CapabilityFacets facets = 7;

  string description = 8;
  EmbeddingDescriptor embedding = 9;

  PricingModel pricing = 10;
  AgentStatus status = 11;
  uint64 last_seen_ns = 12;
  uint64 expires_at_ns = 13;

  bytes facet_fingerprint = 14;
  repeated TrustSignature endorsements = 15;

  // Interop facets — optional bridges to neighboring protocols.
  // These are advertised for composition, not required for Kite-native discovery.
  repeated McpServerDescriptor mcp_servers = 17;
  string a2a_agent_card_url = 18;

  bytes signature = 16;
}

message McpServerDescriptor {
  string name = 1;
  string transport = 2;        // "stdio" | "streamable_http"
  string endpoint = 3;         // URL for streamable_http, local command for stdio
  repeated string tools = 4;   // advertised tool names for candidate filtering
  repeated string resources = 5;
  string auth_scheme = 6;      // e.g. "oauth2", "none"
}
```

Notes:

- `embedding.vector` is not the main network routing object
- `facet_fingerprint` must be reproducible from the exact facets and **does not include** the interop facets (mcp_servers, a2a_agent_card_url) so bridges can evolve without re-keying DHT records
- Cards are signed and TTL-bounded
- Endorsements are optional and policy-driven
- Interop facets are optional: a card without MCP or A2A metadata is still a valid Kite capability. The fields exist so Kite peers can also be consumed by MCP hosts (SEP-2127 pattern [32]) or A2A clients without a separate advertisement surface

### Intent and advert split

The system separates the **full intent** from the **intent advert**.

**Intent** — the full task request: human-readable description, structured payload, resource requirements, privacy rules, response expectations.

**IntentAdvert** — the compact discovery object: exact facet slice, model identifier, LSH signatures, reply deadline, signature.

This split reduces gossip overhead, protects privacy in open mesh contexts, and avoids unnecessary payload fan-out.

### Receipt model

Receipts support optional payment settlement, reputation updates, and auditability in trusted deployments. A receipt is signed by the requester and can include: capability used, completion status, agreed price, latency bucket, coarse quality outcome, and disputes or failure codes. The mesh can use receipts as a reputation source without tying itself to one global scoring algorithm.

---

## Discovery Algorithms and Provable Claims

This section draws a clear line between **bounded behavior** and **heuristic behavior**.

### What is provable

**A. Exact DHT routing is Kademlia.** Directory lookups use exact DHT keys; the routing proof story is Kademlia's XOR-metric analysis, not a new semantic routing theory. For bucket size k = 20 and n nodes, expected lookup length is 0.173 × log₂(n) hops; routing table size is O(k × log n) [1][9]. BitTorrent Mainline DHT deployments (10–20 million daily nodes) validate this at the largest scales ever measured [26].

**B. Semantic retrieval has a collision-probability bound.** For normalized vectors `u, v` with angle θ, single-bit random-hyperplane LSH gives [2]:

```text
Pr[h(u) = h(v)] = 1 - theta / pi
```

Concatenating `k` bits per table and using `L` independent tables gives candidate recall:

```text
Recall_same_bucket >= 1 - (1 - p(theta)^k)^L,  where p(theta) = 1 - theta / pi
```

This is not a proof of perfect nearest-neighbor retrieval. It is a proof-backed bound on *candidate recall* under the chosen LSH family. Cross-polytope LSH offers further refinements for angular distance with near-optimal query complexity bounds [7].

**C. Push fan-out can be reasoned about as topic occupancy.** If a slice has `N_slice` published capabilities and topic prefixes are balanced, a prefix of length `ell` has expected occupancy:

```text
E[topic_size] ~= N_slice / 2^ell
```

This lets us pick topic widths that bound expected push audience without doing content filtering inside the mesh.

**D. Local exact scan is fully correct.** Flat scan returns exact cosine ranking for the local cache. There is no approximation to reason about.

### What is not claimed

1. Global nearest-neighbor optimality across the entire mesh
2. Greedy routing convergence in raw embedding space. This is unsound: cosine distance violates the triangle inequality, and Kleinberg's navigable small-world theory shows that greedy routing over high-dimensional lattices requires long-range links with probability P(r) ∝ r^(−d); at d = 384 the critical exponent makes required links essentially local, destroying navigability [10]
3. Distributed HNSW consistency or global recall guarantees
4. Cross-model cosine comparability
5. One universal threshold for all semantic slices

These are explicitly outside the MVP proof story. They appear in the research track (§12.3, §12.4) as open questions.

### Parameterization strategy

The initial implementation supports calibration ranges rather than hard-coded defaults.

Recommended starter ranges:

- `k` bits per table: 8..12
- Number of tables `L`: 16..32
- Prefix bits `ell` for topics: 6..10
- Local flat-scan cutoff: start around 10,000, then tune with benchmarks

**Example.** For a slice with ~10,000 records and `k = 10`, one exact same-bucket table has expected occupancy ~10,000 / 2^10 ≈ 9.8 candidates before multi-probe. With `L = 32`, collision probability is very high for similar pairs and still useful for moderately similar ones. The tradeoff is benchmarked per slice, not hard-coded into marketing copy.

### Threshold calibration

Each slice has a calibration dataset of labeled `(intent, capability)` pairs with outcomes: *valid match*, *borderline / suggest only*, *invalid match*. From this dataset the system computes `suggest_threshold(slice)` and `accept_threshold(slice)`.

An optional future enhancement is monotonic score calibration that maps cosine to estimated success probability per slice, making product behavior measurable and improvable from real data.

---

## Local Runtime, Transport, and Security

### Local-first runtime

The deployment model has three tiers. Tier 1 ships first.

**Tier 1: local subnet.**

- mDNS discovery
- Direct capability exchange
- Local cache and local search
- State channel over libp2p streams

Two agents on the same machine or LAN collaborate with minimal setup and no DHT dependency.

**Tier 2: trust mesh.**

- PSK-gated transport
- Kademlia directory participation
- Circuit relays as needed
- Signed endorsements and optional trust-depth policies

**Tier 3: global swarm.**

- Public bootstrap nodes
- Directory publication policies
- Payment rails
- Anti-spam controls
- Reputation from signed receipts

The public swarm matters, but it is not the first value-capture point. The first value-capture point is local and trusted collaboration.

### Security model

**Identity.** Each agent has an Ed25519 keypair generated on first start. Ed25519 provides 128-bit security (FIPS 186-5 approved) with 32-byte public keys and 64-byte deterministic signatures [16]. Unoptimized implementations deliver 12,000–30,000 verifications per second per core; SIMD implementations reach millions [27]. Ed25519's deterministic nonce generation eliminates the RNG-failure class that has broken ECDSA implementations in practice.

**Transport.** Connections use libp2p's Noise protocol (XX pattern with Curve25519/ChaChaPoly/SHA256). This has been formally verified through four independent efforts: Noise Explorer (ProVerif) [17], Noise* (F*) [18], Tamarin prover analysis [19], and computational proofs via fACCE models (PKC 2020) [20]. Proven properties include mutual authentication, forward secrecy, identity hiding, and key-compromise-impersonation resistance. Trust domains may add PSK at the transport layer.

**NAT traversal.** A 2025 libp2p measurement study of 4.4 million hole-punching attempts across 85,000 networks in 167 countries reports ~70% ± 7.1% success; TCP and QUIC achieve statistically indistinguishable rates, 97.6% of successful connections establish on the first attempt, and circuit relay adds roughly one RTT of latency [21].

**Integrity.** Capability cards, directory records, intents, proposals, and receipts are all signed.

**Threats and mitigations:**

| Threat | Mitigation |
|---|---|
| Capability spoofing | signed cards, signed directory records, endorsements |
| DHT pollution | TTLs, signature checks, per-peer rate limits, optional trust gate |
| Topic spam | rate limits, topic quotas, light adverts only, block lists |
| Sybil behavior in public mode | proof-of-work / stake / escrow options, reputation from receipts |
| Replay | signed timestamps, expirations, message IDs |
| Unauthorized remote discovery in private deployments | PSK trust mesh and policy gating |

### HNSW as local optimization

HNSW is valuable for local speed at larger cardinalities [3]. It is not used to justify global semantic behavior. The separation is clean:

- **Global correctness story**: exact DHT lookup + LSH recall bounds
- **Local performance story**: flat scan or HNSW, depending on scale

This is easier to explain, benchmark, and defend.

---

## Kite Product Integration

Kite is the first and most opinionated commercial client for the mesh.

### What Kite adds on top of the open mesh

1. **Managed trust infrastructure** — PSK lifecycle, trust-group membership, endorsement management, revocation, audit history
2. **Webhook-to-mesh bridge** — external events become structured mesh intents or informs
3. **Fleet management dashboard** — agent inventory, capability registry, topology, activity, receipts, spend
4. **Calibration and observability** — threshold management, slice quality dashboards, candidate funnel analytics, false-positive review queues
5. **Hosted bootstrap / relay / directory operations** — optional managed operations for teams that want less infrastructure burden

### Product positioning

> Kite Mesh does not claim magical semantic routing.
> It provides structured decentralized discovery using exact filters, probabilistic candidate indexing, and local verification.

### Product surfaces

**Dashboard.** Fleet Overview, Capability Registry, Trust Graph, Discovery Funnel, Topology / Relay Health, Receipt and Reputation Ledger, Threshold Calibration Console.

**CLI.**

```bash
kite mesh start
kite mesh status
kite mesh peers
kite mesh cap publish
kite mesh cap search "sandboxed python with gpu"
kite mesh intent broadcast "run this code in a container"
kite mesh thresholds list
kite mesh thresholds set code-execution/python --suggest 0.68 --accept 0.82
kite mesh trust add <peer_id>
kite mesh topology
```

**Metrics.**

```text
kite_mesh_directory_records_total
kite_mesh_directory_lookup_seconds
kite_mesh_topic_adverts_total
kite_mesh_topic_fetches_total
kite_mesh_candidates_total
kite_mesh_match_accept_total
kite_mesh_match_suggest_total
kite_mesh_false_positive_total
kite_mesh_local_index_mode{mode="flat|hnsw"}
kite_mesh_card_refresh_total
kite_mesh_receipts_total
```

---

## Local Daemon

A single Rust binary.

**Runtime responsibilities:** libp2p transport runtime, local embedding engine, local card cache, flat or HNSW local index, DHT directory publisher/refresher, topic subscription manager, local API for the host agent, metrics endpoint, receipt/persistence store.

**Scope boundaries:** the daemon does not perform greedy semantic routing through the overlay, does not require a distributed ANN graph, and does not support multiple embedding models in production mode.

**Local persistent state:** keypair, peer metadata, capability cards, directory refresh schedule, thresholds, receipts, DLQ / retries, optional local benchmark corpus.

---

## SDK Design

The SDK is simple and opinionated.

### Core API shape

```rust
let client = MeshClient::builder()
    .name("python-worker")
    .tier_local()
    .tier_trust("base64-psk")
    .build()
    .await?;

client.publish_capability(capability_card).await?;

let result = client.broadcast_intent(intent).await?;
```

### SDK behavior

`publish_capability()` computes: facet fingerprint, canonical embedding, LSH table entries, directory records, topic subscriptions.

`broadcast_intent()` returns a richer result: push candidates, pull candidates, final accepted match, rerank explanations, threshold slice used.

Developers can enable `explain_match=true` to see: exact filters passed / failed, cosine score, threshold used, why a candidate was suggested but not auto-accepted. This is valuable for product debugging and trust.

---

## Phasing and Milestones

### Phase 00 — Walking Skeleton

Before any benchmark harness, parameter sweep, or calibration corpus exists, the protocol has to prove its wires connect. The Walking Skeleton (in Cockburn's sense) is the thinnest end-to-end slice that exercises every load-bearing architectural choice in one continuous flow — not to prove discovery works well, but to prove the architecture is buildable, testable, and deployable.

**What it proves.** Two Rust agents on the same laptop start fresh, mint Ed25519 identities, discover each other over mDNS, Noise-handshake, publish and retrieve a signed capability through the canonical-encoder → facet-fingerprint → LSH-directory pipeline, open an ACL conversation (REQUEST → PROPOSE → AGREE → INFORM), and close it with a signed Receipt. Every subsequent phase builds on boxes the skeleton has already wired together.

**Scope — in.**

1. **Identity.** Ed25519 keypair generated and persisted on first boot.
2. **Transport.** libp2p (TCP), Noise XX handshake, direct peer-to-peer dials.
3. **Discovery Tier 1 only.** mDNS peer discovery on the local subnet; no PSK, no bootstrap nodes, no relay.
4. **Canonical encoder.** MiniLM-L6-v2 INT8 loaded via ONNX Runtime; L2-normalized 384-dim output.
5. **One capability card.** Single `CapabilityCard` with real facets (`tools`, `ontologies`, `sandbox_level`, `region`, `status`), real facet fingerprint, one embedding, `L = 4` LSH tables with `k = 8` bits per table. Signed and TTL-bounded.
6. **Directory publication.** libp2p Kademlia in single-swarm mode (two peers is enough). For each LSH table, publish one `DirectoryRecord` under the exact key `Hash(proto_ver || F || model_id || i || h_i(v))`. No GossipSub in the skeleton — pull path only.
7. **Query flow.** Intent is built in-process: exact facets + description → canonical embedding → `L` LSH codes → `L` Kademlia lookups → union candidate set → fetch card → verify signature → facet filter → exact cosine rerank (flat scan over the single candidate) → top match.
8. **ACL negotiation.** Conversation-scoped ACL envelopes carrying `REQUEST` → `PROPOSE` → `AGREE` → `INFORM` with a trivial payload (echo string). Every message signed; `in_reply_to` and `conversation_id` populated correctly.
9. **Receipt.** One signed `Receipt` written to each agent's local SQLite store on success.
10. **Integration test.** Single `cargo test --test walking_skeleton` spins up both daemons in-process, runs the full flow, and asserts the receipt row exists on both sides.
11. **CI.** GitHub Actions pipeline that (a) builds both binaries, (b) runs the integration test, (c) publishes a container image tagged with the commit SHA. CI green is the skeleton's heartbeat.
12. **Observability.** One Prometheus endpoint per daemon exposing `kite_mesh_directory_records_total`, `kite_mesh_directory_lookup_seconds`, `kite_mesh_match_accept_total`, `kite_mesh_receipts_total`. The integration test asserts each counter advanced.

**Scope — out.**

- GossipSub, hash-prefix topics, push path (Phase 1).
- Multiple capabilities, real rerank competition, threshold calibration (Phase 0 / Phase 1).
- HNSW local index (Phase 1; flat scan over one card is trivially correct here).
- PSK trust mesh, Tier 2, relay, NAT traversal (Phase 2).
- Public bootstrap, reputation, payments (Phase 3).
- A2A / MCP / AGNTCY bridges (Phase 4).
- Dashboard, CLI beyond `kite mesh start`, SDK ergonomics beyond what the integration test needs.
- Real benchmark corpus — one hand-crafted `(intent, capability)` pair is enough to close the loop.

**Definition of done.**

1. `cargo test --test walking_skeleton` passes locally and in CI on a clean clone.
2. The test completes in under 10 s end-to-end (encoder warm-start dominates; the discovery + ACL loop should be sub-100 ms after warm-up).
3. Both agents' receipt stores contain the matching conversation's `Receipt`, signatures verify against the peers' Ed25519 public keys.
4. Prometheus counters on both daemons show non-zero values for each of the four metrics listed above.
5. The published container image boots to steady state with `RUST_LOG=info` and no `ERROR` lines.
6. A single markdown runbook (`docs/walking_skeleton.md`) describes how to run the skeleton locally in under three commands.

**Why this is the right skeleton.** Every load-bearing decision in the Executive Summary is exercised: facet-first capability modeling (#1), angular LSH directory over exact DHT keys (#2), local rerank after candidate retrieval (#4), one canonical encoder per epoch (#5). The one deferred commitment — hash-prefix GossipSub topics (#3) — is replaced with the pull path, which is cheaper to implement and exercises the same DirectoryRecord schema. If the skeleton passes, the architecture is proven buildable; if it fails, the failure surfaces in a 2–3 week window rather than halfway through Phase 1.

**Deliberate non-claims.** The skeleton does not validate discovery quality, does not calibrate thresholds, does not measure recall, does not exercise NAT traversal, does not prove anything about multi-peer routing at scale. It proves the pipe is plumbed end to end. Phase 0 is where the quality work begins.

---

### Phase 0 — Evaluation harness and calibration groundwork

Build the benchmark and labeling pipeline before locking discovery defaults.

**Deliverables:** canonical capability schema; labeled slice corpus; threshold calibration tooling; parameter sweep harness for `k`, `L`, `ell`; local flat vs HNSW benchmark; synthetic DHT directory simulator.

**Exit criteria:** at least three high-value slices benchmarked end to end; initial thresholds chosen from data, not intuition; documented default parameter ranges.

### Phase 1 — Local-first MVP

Two or more local agents discover each other, match capabilities, negotiate, and execute without public infrastructure.

**Scope:** Tier 1 only; exact facet filtering; canonical encoder; local flat scan default; optional HNSW; ACL negotiation; state channels; signed cards and receipts; simple explainability.

**Deliberate omissions:** no public swarm dependency; no cross-model support; no global semantic claims.

**Definition of done:** local publish / discover / negotiate / execute / receipt flow; p95 local match latency measured and documented; explainable candidate selection; threshold calibration file checked into the repo for MVP slices.

### Phase 2 — Trust mesh directory

Remote discovery across trusted peers.

**Scope:** PSK trust mesh; Kademlia directory publication and lookup; hash-prefix topic adverts; compact directory record fetch path; relay support; dashboard surfaces for trust graph and discovery funnel.

**Definition of done:** remote capability publication refreshes correctly; push and pull paths both function; union candidate set reranks correctly; stale records expire cleanly; discovery remains explainable.

### Phase 3 — Public swarm and marketplace

Open the public discovery layer without compromising operational sanity.

**Scope:** public bootstrap nodes; managed relay options; reputation from signed receipts; payment rails; anti-spam policy; public directory governance policy.

**Definition of done:** bounded operational cost for directory publication; abuse controls exercised in load tests; payment and receipt loop demonstrated; public-mode reputation is visible but not overclaimed.

### Phase 4 — Ecosystem and bridges

Language bindings; hosted bootstrap and relay program; community reference agents; research-track feature flags.

**Explicit interop bridges:**

- **Kite ↔ A2A gateway.** A standalone service that (a) publishes any Kite peer as an A2A agent by translating its capability card into an Agent Card at `/.well-known/agent-card.json` and mapping ACL performatives onto the Task lifecycle; and (b) represents any reachable A2A agent as a Kite capability card, allowing A2A agents to be discovered through mesh push/pull paths. Uses the interop facets on CapabilityCard (§5) as the translation schema.
- **Kite ↔ MCP bridge.** A thin MCP server that exposes a Kite peer's `mcp_servers` facet contents as proxied MCP tools, so MCP hosts (Claude Desktop, Cursor, VS Code) can consume mesh-discovered agents as ordinary MCP endpoints. Inverse direction: a Kite publisher that announces a local MCP server as a capability card.
- **Kite ↔ AGNTCY facet translator.** Bidirectional mapping between Kite directory records and OASF records so the Cisco AGNTCY directory and a Kite mesh can share capability publications.

**Definition of done:** round-trip interop demonstrated for A2A, MCP, and AGNTCY; the same agent can be discovered and invoked from each peer's native client without Kite-specific knowledge on either side.

---

## Risks and Open Research Questions

### Product risks

| Risk | Severity | Response |
|---|---|---|
| Discovery quality is weak in some slices | High | invest in labeling, facets, threshold calibration, explainability |
| Operators expect one magic threshold | Medium | publish slice-specific defaults and rationale |
| Public swarm abuse becomes expensive | High | gate publication, require receipts, control relay usage |
| Model lock-in is unpopular | Medium | document why canonical encoding is necessary now; keep alignment as research |
| Too much complexity lands before local value is proven | High | keep local-first and trust-mesh first |

### Open technical questions

1. How much capability meaning can be moved from free text into exact facets without making the schema brittle?
2. What is the best per-slice parameter regime for `k`, `L`, and topic prefix width?
3. When does flat scan stop being the right local default on real hardware?
4. How should public reputation combine receipts, endorsements, and failure evidence?
5. What privacy budget should intent adverts expose by default?
6. How should long-running state channels recover after process restart?

### Prior work

No published peer-reviewed system combines HNSW, GossipSub, Kademlia, angular LSH, and semantic embeddings with a local-first tier model and FIPA ACL performatives into a unified agent-discovery mesh. The closest published works each address a fragment:

- **Cisco AGNTCY Agent Directory Service** (IETF `draft-mp-agntcy-ads`, arXiv 2509.18787) [31] is the closest architectural cousin. It uses a **Kademlia-based DHT with a two-level capability → content mapping** and an OASF record format that embeds MCP-server and A2A-handshake descriptors as first-class facets. Kite differs in local-first tiering, angular-LSH semantic matching on the data plane (not just a directory index), FIPA ACL performatives as a negotiation layer, and a single libp2p substrate for discovery + messaging (AGNTCY layers gRPC/SLIM on a separate DHT).
- **Semantica** (arXiv, February 2025) uses LLM embeddings for decentralized search via a tree-structured overlay, demonstrating that "accuracy and speed losses due to decentralization can be mitigated using semantics" [8]. It uses neither HNSW, GossipSub, nor explicit negotiation semantics.
- **Semantic Overlay Networks** literature (2002–2010) combines P2P topology with semantic routing using ontology-based rather than neural-embedding semantics.
- **pSearch** distributed LSI vectors through a Content-Addressable Network DHT, searching only 19 of 128,000 nodes to achieve 91.7% intersection with centralized results — but predates modern neural embeddings by two decades [28].
- **MCP Server Cards (SEP-2127)** [32] propose an HTTP discovery mechanism at `.well-known/mcp/server-card.json` paired with A2A's `.well-known/agent-card.json` under a unified "AI Card" initiative — motivating Kite's decision to carry MCP descriptors as card facets.

Kite Mesh's contribution is a composition that stays inside each component's proven regime while filling a gap — local-first, peer-discoverable, mathematically-grounded agent-to-agent discovery and negotiation — that neither A2A, MCP, ACP, ANP, AGNTCY, NANDA, nor Coral currently occupies.

### Research track A — Projected Cover Overlay

Build a separate semantic overlay using:

1. Normalized embeddings
2. A Johnson-Lindenstrauss random projection to `m = O(log n / epsilon^2)` dimensions [6]
3. A distributed cover-tree / navigating-net structure over projected points [5]
4. Committee-owned overlay nodes located through Kademlia

**Why this is interesting.** JL gives a standard distance-preservation tool [6]. Cover trees give a route to logarithmic nearest-neighbor behavior under bounded intrinsic dimension [5]. Kademlia remains the substrate for finding overlay committees.

**Proposed research claim** (for a paper, not MVP): *Under bounded intrinsic dimension assumptions on the projected semantic space, a distributed cover overlay can support approximate nearest-neighbor discovery with provable candidate guarantees and bounded maintenance cost.*

### Research track B — Alignment Certificates

Permit multiple embedding models without giving up semantic comparability.

Candidate approach: shared public anchor set; orthogonal or constrained alignment map; published error budget; signed certificate admitting a model to a mesh epoch.

---

## Benchmark and Validation Plan

### Evaluation datasets

Build at least three internal gold sets:

1. `code-execution`
2. `web-automation / scraping`
3. `event-routing / webhook / workflow`

Each pair is labeled *accept*, *suggest only*, or *reject*. Without a gold set, threshold choices and slice design drift into opinion.

### Metrics

Measure at minimum: precision@1, recall@k, suggest false-positive rate, auto-accept false-positive rate, candidate count per query, p50/p95 match latency, DHT lookup latency, topic advert fan-out, card refresh overhead, stale record rate, acceptance distribution by slice.

### Required experiments

1. Flat scan vs HNSW locally across 1k, 10k, 100k cards
2. LSH parameter sweeps across `k`, `L`, and multi-probe radius
3. Topic prefix sweep to measure push fan-out
4. Trust-mesh lookup latency under controlled churn
5. Public swarm abuse simulation
6. Replay and stale-record failure modes

### Public claims policy

Until the benchmark harness exists, avoid language like:

- "mathematically closest"
- "provably optimal decentralized semantic routing"
- "0.85 is the right threshold"
- "HNSW is required"
- "multi-model is easy"

Use language like:

- "high-recall candidate generation"
- "exact filters plus probabilistic semantic indexing"
- "per-slice calibrated thresholds"
- "local ANN optimization"

---

## Appendix A — Repository Structure

```text
kite-mesh/
├── Cargo.toml
├── LICENSE
├── README.md
├── crates/
│   ├── kite-mesh-core/
│   │   ├── acl.rs
│   │   ├── capability.rs
│   │   ├── intent.rs
│   │   ├── receipt.rs
│   │   ├── identity.rs
│   │   └── trust.rs
│   │
│   ├── kite-mesh-transport/
│   │   ├── behaviour.rs
│   │   ├── gossipsub_topics.rs
│   │   ├── mdns.rs
│   │   ├── kademlia.rs
│   │   ├── relay.rs
│   │   └── state_channel.rs
│   │
│   ├── kite-mesh-discovery/
│   │   ├── encoder.rs
│   │   ├── facets.rs
│   │   ├── fingerprint.rs
│   │   ├── angular_lsh.rs
│   │   ├── multiprobe.rs
│   │   ├── directory.rs
│   │   ├── topic_prefix.rs
│   │   ├── rerank.rs
│   │   ├── thresholds.rs
│   │   └── explain.rs
│   │
│   ├── kite-mesh-local-index/
│   │   ├── flat.rs
│   │   ├── hnsw.rs
│   │   └── lib.rs
│   │
│   ├── kite-mesh-proto/
│   │   ├── acl.proto
│   │   ├── capability.proto
│   │   ├── directory.proto
│   │   ├── receipt.proto
│   │   └── state_channel.proto
│   │
│   ├── kite-mesh-daemon/
│   │   ├── main.rs
│   │   ├── api.rs
│   │   ├── config.rs
│   │   ├── metrics.rs
│   │   └── persistence.rs
│   │
│   └── kite-mesh-research/
│       ├── projected_cover_overlay.rs
│       └── alignment_certificates.rs
│
└── examples/
    ├── local_swarm/
    ├── trust_mesh_directory/
    ├── explain_match/
    └── threshold_calibration/
```

**Notes:** `angular_lsh.rs` and `directory.rs` are first-class components. `topic_prefix.rs` formalizes how GossipSub topics are derived. `kite-mesh-research/` keeps experimental work isolated from the shipping path.

---

## Appendix B — Configuration Sketch

```toml
[identity]
name = "my-agent"

[network]
listen_addr = "/ip4/0.0.0.0/tcp/4190"
api_addr = "127.0.0.1:4191"
tiers = ["local", "trust"]

[network.local]
mdns_interval_secs = 30

[network.trust]
psk = ""
relay_servers = []

[network.global]
bootstrap_peers = []

[discovery]
model_id = "all-minilm-l6-v2-int8"
mode = "facet_lsh"
lsh_bits_per_table = 10
lsh_tables = 16
topic_prefix_bits = 8
multiprobe_hamming_radius = 1

[local_index]
mode = "auto"                 # flat | hnsw | auto
flat_scan_max_cards = 10000

[thresholds]
default_suggest = 0.65
default_accept = 0.80
calibration_file = "/data/thresholds.toml"

[thresholds.slices."code-execution/python"]
suggest = 0.68
accept = 0.82

[thresholds.slices."web-scraping/browser"]
suggest = 0.64
accept = 0.78

[storage]
data_dir = "/data"
backend = "sqlite"

[metrics]
enabled = true
addr = "0.0.0.0:9191"
```

---

## Appendix C — Mathematical Notes

### C.1 Random-hyperplane LSH

For normalized vectors with angle θ, single-bit collision probability is [2]:

```text
Pr[h(u) = h(v)] = 1 - theta / pi
```

For `k` concatenated bits per table and `L` independent tables:

```text
p = 1 - theta / pi
Recall_same_bucket >= 1 - (1 - p^k)^L
```

This is the right place to make formal recall statements for the discovery layer. Cross-polytope LSH tightens this bound for angular distance at the cost of higher per-hash cost [7].

### C.2 Exact directory lookups

Each directory key is an ordinary DHT key; routing remains in the Kademlia regime rather than a new semantic-metric regime. Expected lookup length is `0.173 × log₂(n)` hops for bucket size k = 20; routing table size is O(k × log n) [1][9].

### C.3 Topic occupancy

Under balanced prefixes within a slice:

```text
E[topic_size] ~= N_slice / 2^ell
```

This is a design knob for expected push fan-out rather than an end-to-end delivery theorem. It composes with GossipSub's O(D × N) message complexity per broadcast within a topic [4][15].

### C.4 Why greedy routing in raw embedding space fails

Three results combine to rule out greedy routing over cosine distance at discovery-scale dimensions:

1. Cosine distance is not a metric — the triangle inequality fails [11][12], breaking the algebraic structure Kademlia's proof depends on.
2. The curse of dimensionality concentrates pairwise distances: at d = 384, the ratio of nearest to farthest distance approaches 1, undermining greedy progress.
3. Kleinberg's navigable small-world theory shows greedy routing achieves O(log² n) hops only when long-range link probability follows P(r) ∝ r^(−d); at d = 384 the critical exponent renders the required distribution essentially local [10].

This is the theoretical basis for routing through exact DHT keys and treating semantics as a directory problem.

### C.5 Research path with projection

Johnson-Lindenstrauss gives a standard path for reducing dimension while approximately preserving distances on finite point sets [6]. Combined with cover-tree reasoning [5], this is the mathematical foundation for the Projected Cover Overlay research track.

---

## Appendix D — References

- **[1]** Petar Maymounkov and David Mazières. *Kademlia: A Peer-to-Peer Information System Based on the XOR Metric.* IPTPS 2002. <https://www.scs.stanford.edu/~dm/home/papers/kpos.pdf>

- **[2]** Moses S. Charikar. *Similarity Estimation Techniques from Rounding Algorithms.* STOC 2002. <https://www.cs.princeton.edu/courses/archive/spr04/cos598B/bib/CharikarEstim.pdf>

- **[3]** Yu. A. Malkov and D. A. Yashunin. *Efficient and robust approximate nearest neighbor search using Hierarchical Navigable Small World graphs.* IEEE TPAMI, 2020. <https://arxiv.org/pdf/1603.09320>

- **[4]** Ankit Kumar, Max von Hippel, Panagiotis Manolios, and Cristina Nita-Rotaru. *Formal Model-Driven Analysis of Resilience of GossipSub to Attacks from Misbehaving Peers.* IEEE S&P 2024. <https://www.ccs.neu.edu/home/pete/pub/sp-2024.pdf>

- **[5]** Alina Beygelzimer, Sham Kakade, and John Langford. *Cover Trees for Nearest Neighbor.* ICML 2006. <https://faculty.cc.gatech.edu/~isbell/reading/papers/cover-tree-icml.pdf>

- **[6]** Sanjoy Dasgupta and Anupam Gupta. *An Elementary Proof of a Theorem of Johnson and Lindenstrauss.* Random Structures & Algorithms, 2003. <https://cseweb.ucsd.edu/~dasgupta/papers/jl.pdf>

- **[7]** Alexandr Andoni, Thijs Laarhoven, Ilya Razenshteyn, and Erik Waingarten. *Practical and Optimal LSH for Angular Distance.* NIPS 2015. <https://people.csail.mit.edu/ludwigs/papers/nips15_crosspolytopelsh.pdf>

- **[8]** Paul Neague et al. *Semantica: Decentralized Search using an LLM-Guided Semantic Tree Network.* arXiv:2502.10151, February 2025. <https://arxiv.org/pdf/2502.10151>

- **[9]** Xing Shi Cai and Luc Devroye. *A Probabilistic Analysis of Kademlia Networks.* Algorithms and Computation — ISAAC 2013, LNCS 8283, pp. 711–721, Springer, 2013. <https://doi.org/10.1007/978-3-642-45030-3_66> (arXiv preprint: <https://arxiv.org/abs/1309.5866>). Journal extension: *The Analysis of Kademlia for Random IDs,* Internet Mathematics 11(6):572–587, 2015. <https://doi.org/10.1080/15427951.2015.1051674>

- **[10]** Jon Kleinberg. *The Small-World Phenomenon: An Algorithmic Perspective.* STOC 2000.

- **[11]** Erich Schubert. *A Triangle Inequality for Cosine Similarity.* SISAP 2021.

- **[12]** John D. Cook. *Cosine similarity does not satisfy the triangle inequality.* <https://www.johndcook.com/blog/2024/06/08/cosine-similarity-triangle-inequality/>

- **[13]** Sentence-Transformers. *all-MiniLM-L6-v2 model card.* Hugging Face. <https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2>

- **[14]** Philipp Schmid. *Accelerate Sentence Transformers with Hugging Face Optimum* (INT8 benchmarks on AWS c6i.xlarge). <https://www.philschmid.de/optimize-sentence-transformers>

- **[15]** libp2p Specifications. *GossipSub v1.1.* <https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.1.md>

- **[16]** NIST. *FIPS 186-5: Digital Signature Standard (DSS).* 2023. <https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.186-5.pdf>

- **[17]** Nadim Kobeissi, Georgio Nicolas, and Karthikeyan Bhargavan. *Noise Explorer: Fully Automated Modeling and Verification for Arbitrary Noise Protocols.* IEEE EuroS&P 2019.

- **[18]** Benjamin Lipp, Bruno Blanchet, and Karthikeyan Bhargavan. *A Mechanised Cryptographic Proof of the WireGuard Virtual Private Network Protocol* / Noise* formal verification work. IEEE S&P 2019.

- **[19]** Guillaume Girol. *Formalizing and Verifying the Security Protocols from the Noise Framework.* Master's thesis, ETH Zürich, 2019. <https://ethz.ch/content/dam/ethz/special-interest/infk/inst-infsec/information-security-group-dam/research/software/ma-19-girol-noise.pdf>

- **[20]** Benjamin Dowling, Paul Rösler, and Jörg Schwenk. *Flexible Authenticated and Confidential Channel Establishment (fACCE).* PKC 2020.

- **[21]** Dennis Trautwein, Cornelius Ihle, Moritz Schubotz, and Bela Gipp. *Challenging Tribal Knowledge — Large Scale Measurement Campaign on Decentralized NAT Traversal.* arXiv preprint arXiv:2510.27500, October 2025. <https://arxiv.org/abs/2510.27500>. Measured 4.4 million traversal attempts across 85,000 networks in 167 countries; DCUtR success rate 70% ± 7.1%, 97.6% of successful connections land on the first attempt.

- **[22]** Weaviate. *Vector index types: HNSW and flat.* <https://weaviate.io/developers/weaviate/concepts/vector-index>

- **[23]** ann-benchmarks.com. *Benchmarks for Approximate Nearest Neighbor algorithms.*

- **[24]** MDPI. *A comprehensive 2025 study of optimal semantic similarity thresholds across models and tasks.*

- **[25]** Sentence-Transformers. *Semantic Textual Similarity benchmarks on Quora Question Pairs (all-mpnet-base-v2).* <https://www.sbert.net/docs/pretrained_models.html>

- **[26]** Raúl Jiménez, Flutra Osmani, and Björn Knutsson. *Sub-Second Lookups on a Large-Scale Kademlia-Based Overlay.* IEEE P2P 2011. Measurements of BitTorrent Mainline DHT at 10–20M daily users. <https://people.kth.se/~rauljc/p2p11/jimenez2011subsecond.pdf>

- **[27]** Daniel J. Bernstein, Niels Duif, Tanja Lange, Peter Schwabe, and Bo-Yin Yang. *High-speed high-security signatures.* Journal of Cryptographic Engineering, 2012. <https://ed25519.cr.yp.to/ed25519-20110926.pdf>

- **[28]** Chunqiang Tang, Zhichen Xu, and Sandhya Dwarkadas. *Peer-to-peer information retrieval using self-organizing semantic overlay networks.* SIGCOMM 2003. <https://dl.acm.org/doi/10.1145/863955.863976>

- **[29]** Model Context Protocol specification and governance. Linux Foundation Agentic AI Foundation (AAIF), December 2025. <https://modelcontextprotocol.io> (official registry preview: <https://registry.modelcontextprotocol.io>)

- **[30]** Agent2Agent (A2A) Protocol specification. Linux Foundation Agent2Agent Project (donated by Google June 2025; absorbed IBM ACP August 2025). <https://github.com/a2aproject/A2A>

- **[31]** Cisco AGNTCY Agent Directory Service. *Agent Directory Service (ADS): A Decentralized Directory for Agentic Systems.* IETF Internet-Draft `draft-mp-agntcy-ads`; arXiv:2509.18787. <https://arxiv.org/abs/2509.18787>

- **[32]** MCP Server Cards. Model Context Protocol Specification Enhancement Proposal SEP-2127. <https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2127>

- **[33]** Google A2A team. *A2A and MCP.* Official A2A documentation. <https://github.com/a2aproject/A2A/blob/main/docs/topics/a2a-and-mcp.md>

- **[34]** *Agent Trust Fabric: A Survey of Identity, Discovery, and Trust Management for Agentic Systems.* arXiv:2507.07901, 2025. <https://arxiv.org/abs/2507.07901>
