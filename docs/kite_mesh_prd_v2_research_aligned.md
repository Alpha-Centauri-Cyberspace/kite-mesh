# Kite Agent Mesh — Research-Aligned PRD (Rewrite)

> **Status**: Draft v2.0  
> **Date**: April 13, 2026  
> **Author**: OpenAI rewrite of the original PRD for expansion by the Kite team  
> **Scope**: Open-source mesh protocol, self-hostable daemon, SDK, and Kite commercial integration  
> **This version supersedes**: the v1 semantic-routing design where "Vector DHT" routing was part of the core correctness story.

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Why This Rewrite Exists](#why-this-rewrite-exists)
3. [What Changed from v1](#what-changed-from-v1)
4. [Product Thesis, Goals, and Non-Goals](#product-thesis-goals-and-non-goals)
5. [System Overview](#system-overview)
6. [Core Architecture](#core-architecture)
7. [Protocol and Data Model](#protocol-and-data-model)
8. [Discovery Algorithms and Provable Claims](#discovery-algorithms-and-provable-claims)
9. [Local Runtime, Transport, and Security](#local-runtime-transport-and-security)
10. [Kite Product Integration](#kite-product-integration)
11. [Self-Hostable Daemon](#self-hostable-daemon)
12. [SDK Design](#sdk-design)
13. [Phasing and Milestones](#phasing-and-milestones)
14. [Risks, Open Questions, and Research Track](#risks-open-questions-and-research-track)
15. [Benchmark and Validation Plan](#benchmark-and-validation-plan)
16. [Appendix A — Revised Repository Structure](#appendix-a--revised-repository-structure)
17. [Appendix B — Configuration Sketch](#appendix-b--configuration-sketch)
18. [Appendix C — Mathematical Notes](#appendix-c--mathematical-notes)
19. [Appendix D — References](#appendix-d--references)

---

## Executive Summary

Kite Agent Mesh remains a strong product and protocol direction. The rewrite is not a retreat from the original vision. It is a change in **what we claim**, **what we prove**, and **what we defer to research**.

The key design decision in this version is simple:

**Kademlia stays responsible for exact key-based routing. Semantic discovery becomes a directory problem, not a routing-metric problem.**

That means this PRD removes "Vector DHT" as an MVP claim and replaces it with a provable hybrid:

1. **Facet-first capability modeling**  
   Every capability is split into:
   - **exact facets**: tools, ontologies, resource guarantees, pricing band, trust tier, network/sandbox requirements, region, status
   - **soft semantics**: a normalized embedding of the human-readable description

2. **Angular LSH directory over exact DHT keys**  
   Capabilities are indexed by exact DHT keys built from:
   - canonicalized facet fingerprint
   - model identifier
   - an angular LSH code derived from the capability embedding

3. **Hash-prefix GossipSub topics**  
   We do **not** do semantic filtering inside a GossipSub mesh. Instead, we create coarse semantic topics from LSH hash prefixes and keep ordinary GossipSub behavior inside each topic.

4. **Local reranking only after candidate retrieval**  
   The system first finds a candidate set with exact filters plus LSH collisions, then reranks locally with exact cosine similarity and policy constraints.

5. **Local ANN is an optimization, not a proof surface**  
   Flat scan is the default local index under modest cardinalities. HNSW becomes an optional local optimization when the card count is high enough to justify it.

6. **One canonical encoder in production phases**  
   Multi-model embedding interoperability is explicitly out of MVP scope. Cross-model alignment is treated as future research.

This gives Kite a product that is still decentralized, local-first, commercially expandable, and mathematically grounded in known results from Kademlia, random-hyperplane LSH, and standard pub/sub design [R1][R2][R4].

---

## Why This Rewrite Exists

The original PRD correctly identified a real opportunity: agents need decentralized discovery, negotiation, and collaboration. The issue was not product vision. The issue was the original proof target.

The earlier design tried to make a Kademlia-like overlay route directly on semantic distance. That is where the theory breaks. Kademlia's lookup guarantees are tied to the XOR metric and its algebraic structure. Once routing is driven by embedding distance, those guarantees no longer follow from the same proof path [R1].

The evaluation also surfaced four practical issues that this rewrite addresses directly:

- a single global similarity threshold is not appropriate across all ontologies and tools
- semantic filtering inside GossipSub topics creates correctness risk
- distributed HNSW should not be part of the correctness story
- multi-model embeddings are not comparable without alignment

This rewrite keeps the architecture ambitious, but it separates:

- **what is shippable**
- **what is provable today**
- **what belongs in a research track**

---

## What Changed from v1

| v1 concept | v2 replacement | Why |
|---|---|---|
| Vector DHT routes by embedding distance | Standard Kademlia routes exact keys; semantics live in an LSH-backed directory | Kademlia's proof depends on XOR routing, not cosine routing [R1] |
| Single global cosine threshold (for example 0.85) | Slice-calibrated thresholds per ontology/tool/resource class | Discovery and near-duplicate detection are different tasks |
| Semantic filtering inside a GossipSub topic | Hash-prefix topics plus pull-on-demand | Preserves ordinary pub/sub behavior inside each mesh [R4] |
| HNSW as part of global discovery story | HNSW local only; exact or flat local search remains valid fallback | Removes distributed graph-consistency problem |
| Raw embedding vector in DHT/gossip as the main primitive | LSH signatures and exact facet fingerprints are the main network primitive | Smaller metadata, better privacy, better proof path |
| Multi-model embeddings left open | Canonical mesh encoder per epoch; alignment deferred | Keeps cosine meaning stable in production |
| "Find the mathematically closest agent on the network" | "Find a high-recall candidate set, then rerank locally" | Honest and defensible claim |

---

## Product Thesis, Goals, and Non-Goals

### Thesis

The mesh should feel like **TCP/IP for agent collaboration**, but it should be built from components that each stay inside their valid mathematical regime.

### Primary goals

1. **Local-first by default**  
   Two agents on the same machine or LAN should discover and collaborate with zero central coordinator.

2. **Semantic discovery without semantic routing claims**  
   Discovery should feel intent-based to users, while the implementation stays grounded in exact key lookup and bounded candidate generation.

3. **Composable trust**  
   Identity, signed capability publication, PSK trust domains, endorsements, and optional public reputation must coexist.

4. **Good product fit for Kite**  
   Kite should become the easiest operational layer for running the mesh in real teams and real event-driven workflows.

5. **A clean research runway**  
   The production system should leave room for novel follow-on work such as projected metric overlays and alignment certificates.

### Non-goals for v2 MVP

1. Proving greedy routing in raw embedding space  
2. Supporting heterogeneous embedding models in production discovery  
3. Building a globally consistent distributed HNSW graph  
4. Promising one universal similarity threshold across all domains  
5. Storing large capability cards directly as heavy DHT values  
6. Making the global public swarm the critical path for early product value

---

## System Overview

The system is divided into three planes.

### 1. Control plane

This plane handles identity, trust, peer connectivity, transport setup, relays, and peer liveness.

Core technologies:
- Ed25519 identity
- Noise transport security
- mDNS for local discovery
- libp2p Kademlia for exact-key directory lookup
- circuit relay / NAT traversal for remote peers

### 2. Discovery plane

This plane answers: *who can do this task?*

Core mechanisms:
- capability cards with exact facets plus embeddings
- angular LSH signatures over normalized embeddings
- Kademlia directory records keyed by `(facet_fingerprint, model_id, table_id, hash_code)`
- hash-prefix GossipSub topics for push-style discovery
- local reranking based on exact cosine, constraints, and policy

### 3. Execution plane

This plane answers: *once we found a match, how do two agents work together?*

Core mechanisms:
- ACL request / propose / agree / inform protocol
- dedicated state channels after matching
- result receipts, optional billing, and reputation signals

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

## 6.1 Facet-first capability model

A capability is not represented as "just an embedding."

Each capability is split into two parts:

### Exact facets

These are fields that should behave like structured filters rather than soft semantics.

Examples:

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

### Soft semantic description

This is the free-text part, such as:

> "Executes Python 3.12 in a sandboxed Linux container with optional CUDA access and a 60-second timeout."

This text is embedded with the canonical encoder. The result is L2-normalized and used only for:

- LSH code generation
- local similarity reranking

**Principle:** exact requirements should not be approximated by embeddings if they can be represented structurally.

---

## 6.2 Canonical mesh encoder

The mesh uses one canonical embedding model per production epoch.

### Production rule

A capability card is semantically eligible for automatic matching only if:

- `model_id` is supported by the node
- the embedding dimension matches the mesh epoch
- the vector is normalized and signed as part of the card

### Why this rule exists

Cosine similarity is only meaningful when vectors come from the same embedding geometry. Supporting heterogeneous models in MVP would make the central score ambiguous and brittle.

### Future path

Cross-model discovery is deferred to a research track built around:

- shared public anchor corpora
- orthogonal or near-orthogonal alignment maps
- signed alignment certificates with explicit error budgets

That work is promising, but it should not block the production design.

---

## 6.3 Angular LSH directory

This is the core replacement for the original Vector DHT idea.

For each capability `c`, define:

- `F(c)` = canonical facet fingerprint
- `v(c)` = normalized embedding
- `h_i(v(c))` = the `i`th angular LSH code for `v(c)`

For each of `L` hash tables, publish a small signed directory record under the exact DHT key:

```text
K_i(c) = Hash(protocol_version || F(c) || model_id || i || h_i(v(c)))
```

The value stored in the DHT is a compact `DirectoryRecord`, not the full capability card.

### Why this works better

- Kademlia still routes exact keys with its usual logic [R1]
- semantics are expressed through collisions in an LSH family [R2]
- the candidate recall is governed by known probability bounds
- capability cards can be refreshed, revoked, and re-published without changing the overlay routing metric

### Directory record contents

A `DirectoryRecord` contains:

- `capability_id`
- `agent_id`
- `peer_id`
- `facet_fingerprint`
- `model_id`
- `table_id`
- `hash_code`
- `status`
- `ttl`
- `signature`

The full card is fetched separately over a direct protocol or from cache.

---

## 6.4 Hash-prefix topics for push discovery

We preserve GossipSub by **not filtering messages inside a topic based on semantic opinions**.

Instead, each node subscribes to coarse topics derived from the same LSH family:

```text
mesh/<ontology>/<tool-class>/<table-id>/<prefix-l-bits>
```

Where:

- `ontology` and `tool-class` come from exact facets
- `table-id` is the LSH table index
- `prefix-l-bits` is a short prefix of the full LSH code

### Publishing

An intent produces a compact `IntentAdvert` containing:

- `intent_id`
- `requester_peer_id`
- exact facet slice
- `model_id`
- per-table LSH signatures or the relevant table signature
- timeout / reply deadline
- signature

The advert goes to the matching prefix topics.

### Receiving

A peer receives the advert because it subscribed to that prefix topic. It then decides whether to:

- ignore it
- request the full intent
- respond with a proposal

That decision is local. The key point is that **the topic mesh itself remains ordinary GossipSub**. We are selecting topics, not redefining message-forwarding correctness inside a topic [R4].

---

## 6.5 Query path

When an agent needs help, the discovery path is:

1. Canonicalize exact requirements into a facet slice
2. Embed the free-text description with the canonical encoder
3. Compute `L` LSH codes
4. Start the **push path** by publishing `IntentAdvert` to coarse topics
5. In parallel, start the **pull path** by doing Kademlia lookups for the exact directory keys
6. Union all candidate directory records
7. Fetch full capability cards where needed
8. Verify signatures and liveness
9. Apply exact facet filter
10. Rerank candidates by exact cosine similarity and policy
11. Apply per-slice thresholds
12. Open ACL negotiation with the top candidate(s)

### Matching policy

Each slice has two thresholds:

- `suggest_threshold(slice)`  
  Lower bar for "show or consider this candidate"

- `accept_threshold(slice)`  
  Higher bar for "auto-propose or auto-select this candidate"

This lets code-execution, web-scraping, GPU inference, and billing-related tasks each use thresholds appropriate to their own semantics.

---

## 6.6 Local search policy

The local daemon keeps a cache of capability cards and performs local search before involving broader discovery.

### Default behavior

- Up to `local_flat_scan_max` cards: exact flat scan over normalized vectors
- Above that threshold: optional HNSW for speed
- Flat scan remains available as a correctness fallback at all times

### Why

At modest local cardinalities, exact scan is often simpler and already fast enough. HNSW is valuable, but it should not be required to make the product correct [R3].

---

## Protocol and Data Model

## 7.1 ACL envelope

The ACL remains the mesh's common message envelope.

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
    CapabilityCardV2 capability = 16;
    DirectoryRecord directory_record = 17;
    Receipt receipt = 18;
  }

  TrustContext trust = 20;
}
```

The performative model from v1 still stands:
- `REQUEST`
- `PROPOSE`
- `AGREE`
- `REFUSE`
- `INFORM`
- `FAILURE`
- `CANCEL`
- `SUBSCRIBE`

---

## 7.2 CapabilityCardV2

```protobuf
message CapabilityCardV2 {
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

  bytes signature = 16;
}
```

### Notes

- `embedding.vector` is not the main network routing object
- `facet_fingerprint` must be reproducible from the exact facets
- cards are signed and TTL-bounded
- endorsements stay optional and policy-driven

---

## 7.3 Intent and advert split

The system separates the **full intent** from the **intent advert**.

### Intent

The full task request:
- human-readable description
- structured payload
- resource requirements
- privacy rules
- response expectations

### IntentAdvert

The compact discovery object:
- exact facet slice
- model identifier
- LSH signatures
- reply deadline
- signature

This split helps with:
- privacy
- lower gossip overhead
- less unnecessary payload fanout

---

## 7.4 Receipt model

Receipts support:
- optional payment settlement
- reputation updates
- auditability in trusted deployments

A receipt is signed by the requester and can include:
- capability used
- completion status
- agreed price
- latency bucket
- coarse quality outcome
- disputes / failure codes

The mesh can use receipts as a source of reputation without tying itself to one global scoring algorithm in the MVP.

---

## Discovery Algorithms and Provable Claims

This section draws a clear line between **proved or bounded behavior** and **heuristic behavior**.

## 8.1 What is actually provable now

### A. Exact DHT routing remains Kademlia

Directory lookups use exact DHT keys. The routing proof story is therefore still Kademlia's, not a new semantic routing theory [R1].

### B. Semantic retrieval has a collision-probability bound

For normalized vectors `u, v` with angle `theta`, random-hyperplane LSH gives:

```text
Pr[h(u) = h(v)] = 1 - theta / pi
```

for a single bit [R2].

If we concatenate `k` independent bits per table and use `L` independent tables, then the probability of at least one exact-table collision is:

```text
Recall_same_bucket >= 1 - (1 - p(theta)^k)^L
```

where:

```text
p(theta) = 1 - theta / pi
```

This is not a proof of perfect nearest neighbor retrieval. It is a proof-backed bound on candidate recall under the chosen LSH family.

### C. Push fanout can be reasoned about as topic occupancy

If a slice has `N_slice` published capabilities and topic prefixes are roughly balanced, then a prefix of length `ell` has expected occupancy:

```text
E[topic_size] ~= N_slice / 2^ell
```

This lets us pick topic widths that bound the expected push audience without doing content filtering inside the mesh.

### D. Local exact scan is fully correct

When local search uses flat scan, the returned cosine ranking is exact for the local cache.

---

## 8.2 What is not claimed as proved

1. Global nearest-neighbor optimality across the entire mesh  
2. Greedy routing convergence in raw embedding space  
3. Distributed HNSW consistency or global recall guarantees  
4. Cross-model cosine comparability  
5. One universal threshold for all semantic slices  

These remain explicitly outside the MVP proof story.

---

## 8.3 Parameterization strategy

The initial implementation should not hard-code one "magic" parameter set. It should support calibration ranges.

### Recommended starter ranges

- `k` bits per table: `8..12`
- number of tables `L`: `16..32`
- prefix bits `ell` for topics: `6..10`
- local flat-scan cutoff: start around `10,000`, then tune with benchmarks

### Example intuition

If a slice had about `10,000` records and we chose `k = 10`, then one exact same-bucket table would have expected occupancy around `10,000 / 2^10 ~= 9.8` candidates before multiprobe. With `L = 32` tables, collision probability is already very high for very similar pairs and still useful for moderately similar pairs. The exact tradeoff should be benchmarked per slice instead of hard-coded into marketing copy.

---

## 8.4 Threshold calibration

This PRD removes the idea of a universal semantic threshold.

Each slice gets a calibration dataset of labeled `(intent, capability)` pairs with outcomes such as:

- valid match
- borderline / suggest only
- invalid match

From this dataset we compute:

- `suggest_threshold(slice)`
- `accept_threshold(slice)`

Optional future enhancement:
- monotonic score calibration that maps cosine to estimated success probability per slice

This makes the system measurable and lets product behavior improve with real data.

---

## Local Runtime, Transport, and Security

## 9.1 Local-first runtime

Tier 1 remains the first shipping mode.

### Tier 1: local subnet

Mechanisms:
- mDNS discovery
- direct capability exchange
- local cache and local search
- state channel establishment over libp2p streams

The goal is simple:
two agents on the same machine or LAN should collaborate with minimal setup and no DHT dependency.

### Tier 2: trust mesh

Mechanisms:
- PSK-gated transport
- Kademlia directory participation
- circuit relays as needed
- signed endorsements and optional trust depth policies

### Tier 3: global swarm

Mechanisms:
- public bootstrap nodes
- directory publication policies
- payment rails
- anti-spam controls
- reputation from signed receipts

The public swarm is important, but it is not the first value capture point. The first value capture point is still local and trusted collaboration.

---

## 9.2 Security model

### Identity

Each agent has an Ed25519 keypair generated on first start.

### Transport

Connections use Noise-based transport security. Trust domains may add PSK at the transport layer.

### Integrity

Capability cards, directory records, intents, proposals, and receipts are all signed.

### Threats and mitigations

| Threat | Mitigation |
|---|---|
| Capability spoofing | signed cards, signed directory records, endorsements |
| DHT pollution | TTLs, signature checks, per-peer rate limits, optional trust gate |
| Topic spam | rate limits, topic quotas, light adverts only, block lists |
| Sybil behavior in public mode | proof-of-work / stake / escrow options, reputation from receipts |
| Replay | signed timestamps, expirations, message IDs |
| Unauthorized remote discovery in private deployments | PSK trust mesh and policy gating |

---

## 9.3 Why HNSW moved out of the critical path

HNSW stays in the design because it is valuable for local speed at larger sizes [R3]. But it is no longer used to justify global semantic behavior. This gives us a much cleaner separation:

- **Global correctness story**: exact DHT lookup + LSH recall bounds
- **Local performance story**: flat scan or HNSW, depending scale

That is easier to explain, benchmark, and defend.

---

## Kite Product Integration

Kite remains the first and best commercial client for the mesh.

## 10.1 What Kite adds on top of the open mesh

1. **Managed trust infrastructure**  
   PSK lifecycle, trust-group membership, endorsement management, revocation, audit history

2. **Webhook-to-mesh bridge**  
   External events become structured mesh intents or informs

3. **Fleet management dashboard**  
   Agent inventory, capability registry, topology, activity, receipts, and spend

4. **Calibration and observability**  
   Threshold management, slice quality dashboards, candidate funnel analytics, false-positive review queues

5. **Hosted bootstrap / relay / directory operations**  
   Optional managed operations layer for teams that want less infrastructure burden

---

## 10.2 New product positioning

This version makes the product story stronger because it is easier to explain:

> Kite Mesh does not claim magical semantic routing.
> It provides structured decentralized discovery using exact filters, probabilistic candidate indexing, and local verification.

That is still powerful, and it is more credible.

---

## 10.3 Product surfaces

### Dashboard

New screens:
- Fleet Overview
- Capability Registry
- Trust Graph
- Discovery Funnel
- Topology / Relay Health
- Receipt and Reputation Ledger
- Threshold Calibration Console

### CLI

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

### Metrics

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

## Self-Hostable Daemon

The daemon remains a single Rust binary. The main change is internal architecture.

### 11.1 Runtime responsibilities

- libp2p transport runtime
- local embedding engine
- local card cache
- flat or HNSW local index
- DHT directory publisher / refresher
- topic subscription manager
- local API for the host agent
- metrics endpoint
- receipt / persistence store

### 11.2 What the daemon no longer claims to do

- it does not perform greedy semantic routing through the overlay
- it does not require a distributed ANN graph
- it does not need multiple embedding models in production mode

### 11.3 Storage responsibilities

Local persistent state includes:
- keypair
- peer metadata
- capability cards
- directory refresh schedule
- thresholds
- receipts
- DLQ / retries
- optional local benchmark corpus

---

## SDK Design

The SDK stays simple and opinionated.

### 12.1 Core API shape

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

### 12.2 Important SDK behavior changes

1. `publish_capability()` now computes:
   - facet fingerprint
   - canonical embedding
   - LSH table entries
   - directory records
   - topic subscriptions

2. `broadcast_intent()` now returns a richer result:
   - push candidates
   - pull candidates
   - final accepted match
   - rerank explanations
   - threshold slice used

3. Local developers can turn on `explain_match=true` to understand:
   - exact filters passed / failed
   - cosine score
   - threshold used
   - why a candidate was suggested but not auto-accepted

This is valuable for product debugging and trust.

---

## Phasing and Milestones

## Phase 0 — Evaluation harness and calibration groundwork

**Goal:** Build the benchmark and labeling pipeline before locking discovery defaults.

### Deliverables

- canonical capability schema
- labeled slice corpus
- threshold calibration tooling
- parameter sweep harness for `k`, `L`, `ell`
- local flat vs HNSW benchmark
- synthetic DHT directory simulator

### Exit criteria

- at least three high-value slices benchmarked end to end
- initial thresholds chosen from data, not intuition
- documented default parameter ranges

---

## Phase 1 — Local-first MVP

**Goal:** Two or more local agents discover each other, match capabilities, negotiate, and execute, without relying on public infrastructure.

### Scope

- Tier 1 only
- exact facet filtering
- canonical encoder
- local flat scan default
- optional HNSW
- ACL negotiation
- state channels
- signed cards and receipts
- simple explainability

### Deliberate omissions

- no public swarm dependency
- no cross-model support
- no global semantic claims

### Definition of done

- local publish / discover / negotiate / execute / receipt flow
- p95 local match latency measured and documented
- explainable candidate selection
- threshold calibration file checked into the repo for MVP slices

---

## Phase 2 — Trust mesh directory

**Goal:** Enable remote discovery across trusted peers while keeping the proof story clean.

### Scope

- PSK trust mesh
- Kademlia directory publication and lookup
- hash-prefix topic adverts
- compact directory record fetch path
- relay support
- dashboard surfaces for trust graph and discovery funnel

### Definition of done

- remote capability publication refreshes correctly
- push and pull paths both function
- union candidate set reranks correctly
- stale records expire cleanly
- discovery remains explainable

---

## Phase 3 — Public swarm and marketplace

**Goal:** Open the public discovery layer without compromising operational sanity.

### Scope

- public bootstrap nodes
- managed relay options
- reputation from signed receipts
- payment rails
- anti-spam policy
- public directory governance policy

### Definition of done

- bounded operational cost for directory publication
- abuse controls exercised in load tests
- payment and receipt loop demonstrated
- public-mode reputation is visible but not overclaimed

---

## Phase 4 — Ecosystem and bridges

### Scope

- language bindings
- HTTP / A2A / MCP bridge layers
- hosted bootstrap and relay program
- community reference agents
- research-track feature flags

---

## Risks, Open Questions, and Research Track

## 14.1 Main product risks

| Risk | Severity | Response |
|---|---|---|
| Discovery quality is weak in some slices | High | invest in labeling, facets, threshold calibration, explainability |
| Operators expect one magic threshold | Medium | publish slice-specific defaults and rationale |
| Public swarm abuse becomes expensive | High | gate publication, require receipts, control relay usage |
| Model lock-in is unpopular | Medium | document why canonical encoding is necessary now; keep alignment as research |
| Too much complexity lands before local value is proven | High | keep local-first and trust-mesh first |

---

## 14.2 Open technical questions

1. How much of capability meaning can be moved from free text into exact facets without making the schema brittle?  
2. What is the best per-slice parameter regime for `k`, `L`, and topic prefix width?  
3. When does flat scan stop being the right local default on real hardware?  
4. How should public reputation combine receipts, endorsements, and failure evidence?  
5. What privacy budget should intent adverts expose by default?  
6. How should long-running state channels recover after process restart?

---

## 14.3 Research track A — Projected Cover Overlay

This is the main "new algorithm / new data structure" track and is intentionally separated from the shipping path.

### Idea

Build a separate semantic overlay using:

1. normalized embeddings
2. a Johnson-Lindenstrauss random projection to `m = O(log n / epsilon^2)` dimensions [R6]
3. a distributed cover-tree / navigating-net style structure over projected points [R5]
4. committee-owned overlay nodes located through Kademlia

### Why this is interesting

- JL gives a standard distance-preservation tool [R6]
- cover trees give a route to logarithmic-ish nearest-neighbor behavior under bounded intrinsic dimension [R5]
- Kademlia can still be used as the substrate for finding overlay committees

### Proposed research claim

Not for MVP, but for a paper:

> Under bounded intrinsic dimension assumptions on the projected semantic space, can a distributed cover overlay support approximate nearest-neighbor discovery with provable candidate guarantees and bounded maintenance cost?

This is a real research program. It is not required for the product to succeed.

---

## 14.4 Research track B — Alignment certificates

A second research track focuses on multi-model interoperability.

### Goal

Permit multiple embedding models without giving up semantic comparability.

### Candidate approach

- shared public anchor set
- orthogonal or constrained alignment map
- published error budget
- signed certificate admitting a model to a mesh epoch

Again: promising, but not part of the production critical path.

---

## Benchmark and Validation Plan

This section should exist before any public claims.

## 15.1 Evaluation datasets

Build at least three internal gold sets:

1. `code-execution`
2. `web-automation / scraping`
3. `event-routing / webhook / workflow`

Each pair should be labeled as:
- accept
- suggest only
- reject

### Why this matters

Without a gold set, threshold choices and slice design will drift into opinion.

---

## 15.2 Metrics

Measure at minimum:

- precision@1
- recall@k
- suggest false-positive rate
- auto-accept false-positive rate
- candidate count per query
- p50 / p95 match latency
- DHT lookup latency
- topic advert fanout
- card refresh overhead
- stale record rate
- acceptance distribution by slice

---

## 15.3 Required experiments

1. Flat scan vs HNSW locally across `1k`, `10k`, `100k` cards  
2. LSH parameter sweeps across `k`, `L`, and multiprobe radius  
3. Topic prefix sweep to measure push fanout  
4. Trust-mesh lookup latency under controlled churn  
5. Public swarm abuse simulation  
6. Replay and stale-record failure modes  

---

## 15.4 Public claims policy

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

## Appendix A — Revised Repository Structure

```text
kite-mesh/
├── Cargo.toml
├── LICENSE
├── README.md
├── crates/
│   ├── kite-mesh-core/
│   │   ├── acl.rs
│   │   ├── capability_v2.rs
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
│   │   ├── capability_v2.proto
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

### Notes on repository changes

- `vector_dht.rs` is removed from the production path
- `angular_lsh.rs` and `directory.rs` become first-class components
- `topic_prefix.rs` formalizes how GossipSub topics are derived
- `kite-mesh-research/` keeps experimental work isolated from the shipping path

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

For normalized vectors with angle `theta`:

```text
Pr[h(u) = h(v)] = 1 - theta / pi
```

For `k` concatenated bits per table and `L` independent tables:

```text
p = 1 - theta / pi
Recall_same_bucket >= 1 - (1 - p^k)^L
```

This formula is the right place to make formal recall statements for the discovery layer [R2].

### C.2 Exact directory lookups

Each directory key is an ordinary DHT key, so routing is still in the Kademlia regime rather than a new semantic metric regime [R1].

### C.3 Topic occupancy

Under balanced prefixes within a slice:

```text
E[topic_size] ~= N_slice / 2^ell
```

This is not a full end-to-end theorem about delivery, but it is a clean design knob for expected push fanout.

### C.4 Research path with projection

Johnson-Lindenstrauss gives a standard path for reducing dimension while approximately preserving distances on finite point sets [R6]. Combined with cover-tree-style reasoning [R5], it suggests a serious future path for a new distributed semantic overlay.

---

## Appendix D — References

- **[R1]** Petar Maymounkov and David Mazières, *Kademlia: A Peer-to-Peer Information System Based on the XOR Metric*.  
  <https://www.scs.stanford.edu/~dm/home/papers/kpos.pdf>

- **[R2]** Moses S. Charikar, *Similarity Estimation Techniques from Rounding Algorithms*.  
  <https://www.cs.princeton.edu/courses/archive/spr04/cos598B/bib/CharikarEstim.pdf>

- **[R3]** Yu. A. Malkov and D. A. Yashunin, *Efficient and robust approximate nearest neighbor search using Hierarchical Navigable Small World graphs*.  
  <https://arxiv.org/pdf/1603.09320>

- **[R4]** Ankit Kumar, Max von Hippel, Panagiotis Manolios, and Cristina Nita-Rotaru, *Formal Model-Driven Analysis of Resilience of GossipSub to Attacks from Misbehaving Peers*.  
  <https://www.ccs.neu.edu/home/pete/pub/sp-2024.pdf>

- **[R5]** Alina Beygelzimer, Sham Kakade, and John Langford, *Cover Trees for Nearest Neighbor*.  
  <https://faculty.cc.gatech.edu/~isbell/reading/papers/cover-tree-icml.pdf>

- **[R6]** Sanjoy Dasgupta and Anupam Gupta, *An elementary proof of a theorem of Johnson and Lindenstrauss*.  
  <https://cseweb.ucsd.edu/~dasgupta/papers/jl.pdf>

- **[R7]** Alexandr Andoni, Thijs Laarhoven, Ilya Razenshteyn, and Erik Waingarten, *Practical and Optimal LSH for Angular Distance*.  
  <https://people.csail.mit.edu/ludwigs/papers/nips15_crosspolytopelsh.pdf>

- **[R8]** Paul Neague et al., *Decentralized Search using a LLM-Guided Semantic Tree Network*.  
  <https://arxiv.org/pdf/2502.10151>
