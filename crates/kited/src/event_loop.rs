//! libp2p event loop + ACL conversation state machine.
//!
//! ACL conversation (two round trips on the `AclCodec` request-response protocol):
//!
//! - Round 1: requester sends REQUEST → provider responds with PROPOSE.
//! - Round 2: requester sends AGREE → provider executes and responds with INFORM.
//!
//! Both sides persist a signed Receipt on success.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Instant,
};

use futures::StreamExt;
use kite_mesh_core::{
    AgentIdentity,
    acl::{self as acl_core, AclBuilder},
    capability, receipt,
};
use kite_mesh_discovery::{
    AngularLsh,
    directory::{DirectoryPublication, build_records, lookup_keys, verify_record},
    encoder::CanonicalEncoder,
    rerank::rerank,
};
use kite_mesh_proto::{
    AclMessage, Agreement, CANONICAL_MODEL_ID, CapabilityCard, CapabilityFacets, DirectoryRecord,
    Intent, Performative, Proposal, ResultStatus, TaskResult, acl_message::Payload,
};
use kite_mesh_transport::{
    MeshBehaviour, MeshBehaviourEvent,
    codec::{CardRequest, CardResponse},
};
use libp2p::{
    PeerId, Swarm,
    kad::{self, Quorum, Record, RecordKey},
    request_response,
    swarm::SwarmEvent,
};
use prost::Message;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use crate::{
    config::KitedConfig,
    daemon::{Command, IntentOutcome, KitedEvent},
    metrics as m,
};

const ECHO_ONTOLOGY: &str = "code-execution";
const RECORD_TTL_NS: u64 = 300_000_000_000; // 5 minutes

pub struct EventLoop;

impl EventLoop {
    pub async fn spawn(
        swarm: Swarm<MeshBehaviour>,
        identity: AgentIdentity,
        encoder: CanonicalEncoder,
        lsh: Arc<AngularLsh>,
        receipts: kite_mesh_core::receipt::ReceiptStore,
        config: KitedConfig,
        cmd_rx: mpsc::Receiver<Command>,
        evt_tx: mpsc::Sender<KitedEvent>,
    ) -> anyhow::Result<JoinHandle<anyhow::Result<()>>> {
        let mut state = State {
            swarm,
            identity,
            _encoder: encoder,
            lsh,
            receipts,
            evt_tx,
            cmd_rx,
            local_cards: HashMap::new(),
            pending_lookups: HashMap::new(),
            pending_card_fetches: HashMap::new(),
            requesters: HashMap::new(),
            providers: HashMap::new(),
            pending_publish_acks: HashMap::new(),
            pending_acl_round1: HashMap::new(),
            pending_acl_round2: HashMap::new(),
            known_peers: HashSet::new(),
        };

        let addr = config.network.listen_addr.parse()?;
        state.swarm.listen_on(addr)?;

        Ok(tokio::spawn(async move { state.run().await }))
    }
}

// -- per-conversation state ---------------------------------------------------

/// Requester-side state during the round-trip exchange.
struct RequesterState {
    #[allow(dead_code)]
    conversation_id: String,
    required: CapabilityFacets,
    query_vector: Vec<f32>,
    intent: Intent,
    outstanding_dht_queries: HashSet<kad::QueryId>,
    fetched_cards: Vec<CapabilityCard>,
    seen_capability_ids: HashSet<String>,
    in_flight_card_fetches: HashSet<request_response::OutboundRequestId>,
    request_sent: bool,
    provider_peer: Option<PeerId>,
    chosen_capability_id: Option<String>,
    chosen_score: f32,
    started_at: Instant,
    reply: Option<oneshot::Sender<anyhow::Result<IntentOutcome>>>,
}

/// Provider-side state between REQUEST/PROPOSE and AGREE/INFORM.
struct ProviderState {
    requester_peer: PeerId,
    capability_id: String,
    intent_payload: Vec<u8>,
}

struct PublishAck {
    cap_id: String,
    reply: oneshot::Sender<anyhow::Result<()>>,
    remaining: u32,
}

struct PendingCardFetch {
    conversation_id: String,
}

struct PendingLookup {
    conversation_id: String,
}

struct State {
    swarm: Swarm<MeshBehaviour>,
    identity: AgentIdentity,
    _encoder: CanonicalEncoder,
    lsh: Arc<AngularLsh>,
    receipts: kite_mesh_core::receipt::ReceiptStore,
    evt_tx: mpsc::Sender<KitedEvent>,
    cmd_rx: mpsc::Receiver<Command>,

    local_cards: HashMap<String, CapabilityCard>,
    pending_lookups: HashMap<kad::QueryId, PendingLookup>,
    pending_card_fetches: HashMap<request_response::OutboundRequestId, PendingCardFetch>,

    requesters: HashMap<String, RequesterState>,
    providers: HashMap<String, ProviderState>,

    pending_publish_acks: HashMap<kad::QueryId, PublishAck>,
    /// Outbound request ids awaiting a PROPOSE response.
    pending_acl_round1: HashMap<request_response::OutboundRequestId, String>,
    /// Outbound request ids awaiting an INFORM response.
    pending_acl_round2: HashMap<request_response::OutboundRequestId, String>,

    known_peers: HashSet<PeerId>,
}

impl State {
    async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                cmd = self.cmd_rx.recv() => match cmd {
                    Some(Command::Shutdown) | None => return Ok(()),
                    Some(other) => self.handle_command(other).await?,
                },
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await?;
                }
            }
        }
    }

    fn emit(&self, event: KitedEvent) {
        let _ = self.evt_tx.try_send(event);
    }

    // -- commands -----------------------------------------------------------

    async fn handle_command(&mut self, cmd: Command) -> anyhow::Result<()> {
        match cmd {
            Command::Shutdown => unreachable!(),
            Command::PublishCapability { card, ack } => self.publish(*card, ack).await,
            Command::SubmitIntent {
                required,
                query_vector,
                intent,
                reply,
            } => {
                self.submit_intent(*required, query_vector, *intent, reply)
                    .await
            }
        }
    }

    async fn publish(
        &mut self,
        card: CapabilityCard,
        reply: oneshot::Sender<anyhow::Result<()>>,
    ) -> anyhow::Result<()> {
        capability::verify(&card)?;
        let publications: Vec<DirectoryPublication> =
            build_records(&card, &self.lsh, &self.identity, RECORD_TTL_NS)?;
        let expected = publications.len() as u32;
        let cap_id = card.capability_id.clone();
        self.local_cards.insert(cap_id.clone(), card);

        if publications.is_empty() {
            let _ = reply.send(Ok(()));
            return Ok(());
        }

        // Share a single PublishAck across all put_record query ids via
        // Arc<Mutex<_>>? Simpler: track a counter keyed on the *first* qid only,
        // and leave the others as no-op metric bumps.
        let mut qids: Vec<kad::QueryId> = Vec::with_capacity(publications.len());
        for pub_ in publications {
            let record = Record {
                key: RecordKey::new(&pub_.key),
                value: pub_.record_bytes,
                publisher: Some(*self.swarm.local_peer_id()),
                expires: None,
            };
            let qid = self
                .swarm
                .behaviour_mut()
                .kademlia
                .put_record(record, Quorum::One)?;
            qids.push(qid);
        }

        // Wire the ack against the *last* qid; all prior qids are tracked as
        // "remaining" counters for the metric side.
        let last = qids.pop().expect("non-empty");
        self.pending_publish_acks.insert(
            last,
            PublishAck {
                cap_id,
                reply,
                remaining: expected,
            },
        );
        for qid in qids {
            // Count-only qids.
            self.pending_publish_acks.insert(
                qid,
                PublishAck {
                    cap_id: String::new(),
                    reply: oneshot::channel().0, // throwaway
                    remaining: 0,
                },
            );
        }
        Ok(())
    }

    async fn submit_intent(
        &mut self,
        required: CapabilityFacets,
        query_vector: Vec<f32>,
        intent: Intent,
        reply: oneshot::Sender<anyhow::Result<IntentOutcome>>,
    ) -> anyhow::Result<()> {
        let fp = kite_mesh_core::facets::fingerprint(&required);
        let keys = lookup_keys(&fp, &query_vector, &self.lsh);
        let conversation_id = uuid::Uuid::new_v4().to_string();

        let mut outstanding = HashSet::new();
        for (_table_id, key) in keys {
            let qid = self
                .swarm
                .behaviour_mut()
                .kademlia
                .get_record(RecordKey::new(&key));
            outstanding.insert(qid);
            self.pending_lookups.insert(
                qid,
                PendingLookup {
                    conversation_id: conversation_id.clone(),
                },
            );
        }

        self.requesters.insert(
            conversation_id.clone(),
            RequesterState {
                conversation_id: conversation_id.clone(),
                required,
                query_vector,
                intent,
                outstanding_dht_queries: outstanding,
                fetched_cards: vec![],
                seen_capability_ids: HashSet::new(),
                in_flight_card_fetches: HashSet::new(),
                request_sent: false,
                provider_peer: None,
                chosen_capability_id: None,
                chosen_score: 0.0,
                started_at: Instant::now(),
                reply: Some(reply),
            },
        );

        // If mesh is slow to populate, give a short grace period before DHT
        // lookups drain. We emit the ACL request once dht queries settle.
        Ok(())
    }

    // -- swarm event handling ----------------------------------------------

    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<MeshBehaviourEvent>,
    ) -> anyhow::Result<()> {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                tracing::info!(%address, "swarm listening");
                self.emit(KitedEvent::Listening(address.to_string()));
            }
            SwarmEvent::Behaviour(MeshBehaviourEvent::Mdns(ev)) => self.handle_mdns(ev).await,
            SwarmEvent::Behaviour(MeshBehaviourEvent::Kademlia(ev)) => self.handle_kad(ev).await?,
            SwarmEvent::Behaviour(MeshBehaviourEvent::Identify(ev)) => {
                self.handle_identify(ev);
            }
            SwarmEvent::Behaviour(MeshBehaviourEvent::Card(ev)) => self.handle_card(ev).await?,
            SwarmEvent::Behaviour(MeshBehaviourEvent::Acl(ev)) => self.handle_acl(ev).await?,
            _ => {}
        }
        Ok(())
    }

    fn handle_identify(&mut self, ev: libp2p::identify::Event) {
        if let libp2p::identify::Event::Received { peer_id, info, .. } = ev {
            let speaks_kad = info.protocols.iter().any(|p| p == &kad::PROTOCOL_NAME);
            if speaks_kad {
                for addr in info.listen_addrs {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, addr);
                }
                tracing::debug!(%peer_id, "identify confirmed kad protocol; added addresses");
            }
        }
    }

    async fn handle_mdns(&mut self, ev: libp2p::mdns::Event) {
        use libp2p::mdns::Event::*;
        match ev {
            Discovered(list) => {
                for (peer, addr) in list {
                    // Dial only. We intentionally do NOT call
                    // kademlia.add_address here — doing so triggers a spurious
                    // RoutingUpdated before any connection exists, which makes
                    // put_record fire against an unreachable peer. Instead,
                    // libp2p-identify notifies libp2p-kad of the peer's
                    // supported protocols after the Noise+identify handshake,
                    // and kad adds the peer to the routing table at that point.
                    if let Err(e) = self.swarm.dial(addr.clone()) {
                        tracing::debug!(%peer, error = ?e, "dial on mdns failed");
                    } else {
                        tracing::info!(%peer, %addr, "peer seen via mdns; dialing");
                    }
                }
            }
            Expired(_) => {}
        }
    }

    async fn handle_kad(&mut self, ev: kad::Event) -> anyhow::Result<()> {
        match ev {
            kad::Event::RoutingUpdated {
                peer,
                is_new_peer: true,
                ..
            } => {
                if self.known_peers.insert(peer) {
                    tracing::info!(%peer, "peer added to kademlia routing");
                    self.emit(KitedEvent::PeerDiscovered(peer));
                }
                return Ok(());
            }
            kad::Event::OutboundQueryProgressed { .. } => {}
            _ => return Ok(()),
        }
        let kad::Event::OutboundQueryProgressed { id, result, .. } = ev else {
            return Ok(());
        };
        match result {
            kad::QueryResult::PutRecord(Ok(_)) => {
                metrics::counter!(m::DIRECTORY_RECORDS_TOTAL).increment(1);
                if let Some(ack) = self.pending_publish_acks.remove(&id)
                    && ack.remaining > 0
                {
                    let cap_id = ack.cap_id.clone();
                    let _ = ack.reply.send(Ok(()));
                    self.emit(KitedEvent::CapabilityPublished {
                        capability_id: cap_id,
                        record_count: ack.remaining,
                    });
                }
            }
            kad::QueryResult::PutRecord(Err(e)) => {
                tracing::warn!(error = ?e, "put_record failed");
                if let Some(ack) = self.pending_publish_acks.remove(&id)
                    && ack.remaining > 0
                {
                    let _ = ack.reply.send(Err(anyhow::anyhow!("{e:?}")));
                }
            }
            kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(rec))) => {
                if let Some(lookup) = self.pending_lookups.get(&id) {
                    let conv = lookup.conversation_id.clone();
                    self.on_directory_hit(rec.record, conv).await?;
                }
            }
            kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FinishedWithNoAdditionalRecord {
                ..
            })) => {
                if let Some(lookup) = self.pending_lookups.remove(&id) {
                    if let Some(req) = self.requesters.get_mut(&lookup.conversation_id) {
                        req.outstanding_dht_queries.remove(&id);
                    }
                    self.maybe_send_request(&lookup.conversation_id).await?;
                }
            }
            kad::QueryResult::GetRecord(Err(e)) => {
                tracing::debug!(error = ?e, "get_record terminated");
                if let Some(lookup) = self.pending_lookups.remove(&id) {
                    if let Some(req) = self.requesters.get_mut(&lookup.conversation_id) {
                        req.outstanding_dht_queries.remove(&id);
                    }
                    self.maybe_send_request(&lookup.conversation_id).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn on_directory_hit(
        &mut self,
        record: Record,
        conversation_id: String,
    ) -> anyhow::Result<()> {
        let dir = match DirectoryRecord::decode(record.value.as_slice()) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(error = ?e, "bad directory record payload");
                return Ok(());
            }
        };
        if dir.model_id != CANONICAL_MODEL_ID {
            return Ok(());
        }
        if verify_record(&dir).is_err() {
            tracing::warn!(peer = %dir.peer_id, "directory record signature invalid");
            return Ok(());
        }
        let Some(req) = self.requesters.get_mut(&conversation_id) else {
            return Ok(());
        };
        if !req.seen_capability_ids.insert(dir.capability_id.clone()) {
            return Ok(());
        }
        let Ok(peer) = dir.peer_id.parse::<PeerId>() else {
            return Ok(());
        };
        let req_id = self.swarm.behaviour_mut().card.send_request(
            &peer,
            CardRequest {
                capability_id: dir.capability_id.clone(),
            },
        );
        req.in_flight_card_fetches.insert(req_id);
        self.pending_card_fetches
            .insert(req_id, PendingCardFetch { conversation_id });
        Ok(())
    }

    // -- card codec ---------------------------------------------------------

    async fn handle_card(
        &mut self,
        ev: request_response::Event<CardRequest, CardResponse>,
    ) -> anyhow::Result<()> {
        match ev {
            request_response::Event::Message { message, .. } => match message {
                request_response::Message::Request {
                    request, channel, ..
                } => {
                    let resp = match self.local_cards.get(&request.capability_id).cloned() {
                        Some(card) => CardResponse::Found(card),
                        None => CardResponse::NotFound,
                    };
                    let _ = self.swarm.behaviour_mut().card.send_response(channel, resp);
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    let Some(fetch) = self.pending_card_fetches.remove(&request_id) else {
                        return Ok(());
                    };
                    let conv = fetch.conversation_id;
                    let mut ready = false;
                    if let Some(req) = self.requesters.get_mut(&conv) {
                        req.in_flight_card_fetches.remove(&request_id);
                        if let CardResponse::Found(card) = response
                            && capability::verify(&card).is_ok()
                        {
                            req.fetched_cards.push(card);
                        }
                        ready = req.outstanding_dht_queries.is_empty()
                            && req.in_flight_card_fetches.is_empty();
                    }
                    if ready {
                        self.maybe_send_request(&conv).await?;
                    }
                }
            },
            _ => {}
        }
        Ok(())
    }

    // -- acl round trips ----------------------------------------------------

    async fn maybe_send_request(&mut self, conversation_id: &str) -> anyhow::Result<()> {
        let (peer, request_msg, score, capability_id) = {
            let Some(req) = self.requesters.get_mut(conversation_id) else {
                return Ok(());
            };
            if req.request_sent
                || !req.outstanding_dht_queries.is_empty()
                || !req.in_flight_card_fetches.is_empty()
            {
                return Ok(());
            }
            let scored = rerank(
                &req.required,
                &req.query_vector,
                std::mem::take(&mut req.fetched_cards),
            );
            let Some(top) = scored.into_iter().find(|c| c.passed_filter) else {
                if let Some(reply) = req.reply.take() {
                    let _ = reply.send(Err(anyhow::anyhow!("no matching capability")));
                }
                self.requesters.remove(conversation_id);
                return Ok(());
            };
            let provider: PeerId = top.card.agent_id.parse()?;
            let builder = AclBuilder::new(&self.identity, conversation_id.to_string());
            let msg = builder.request(ECHO_ONTOLOGY, req.intent.clone())?;
            req.request_sent = true;
            req.provider_peer = Some(provider);
            req.chosen_capability_id = Some(top.card.capability_id.clone());
            req.chosen_score = top.score;

            metrics::counter!(m::MATCH_ACCEPT_TOTAL).increment(1);
            metrics::histogram!(m::DIRECTORY_LOOKUP_SECONDS)
                .record(req.started_at.elapsed().as_secs_f64());

            (provider, msg, top.score, top.card.capability_id.clone())
        };
        let _ = (score, capability_id);
        let req_id = self
            .swarm
            .behaviour_mut()
            .acl
            .send_request(&peer, request_msg);
        self.pending_acl_round1
            .insert(req_id, conversation_id.to_string());
        Ok(())
    }

    async fn handle_acl(
        &mut self,
        ev: request_response::Event<AclMessage, AclMessage>,
    ) -> anyhow::Result<()> {
        match ev {
            request_response::Event::Message { peer, message, .. } => match message {
                request_response::Message::Request {
                    request, channel, ..
                } => {
                    self.on_acl_request(peer, request, channel).await?;
                }
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    self.on_acl_response(peer, request_id, response).await?;
                }
            },
            _ => {}
        }
        Ok(())
    }

    async fn on_acl_request(
        &mut self,
        peer: PeerId,
        msg: AclMessage,
        channel: request_response::ResponseChannel<AclMessage>,
    ) -> anyhow::Result<()> {
        if acl_core::verify(&msg).is_err() {
            tracing::warn!(%peer, "acl request signature invalid");
            return Ok(());
        }
        match Performative::try_from(msg.performative).unwrap_or(Performative::Unspecified) {
            Performative::Request => self.handle_request_round1(peer, msg, channel).await,
            Performative::Agree => self.handle_agree_round2(peer, msg, channel).await,
            other => {
                tracing::warn!(?other, "unexpected performative on request channel");
                Ok(())
            }
        }
    }

    async fn handle_request_round1(
        &mut self,
        peer: PeerId,
        msg: AclMessage,
        channel: request_response::ResponseChannel<AclMessage>,
    ) -> anyhow::Result<()> {
        let Some(Payload::Intent(intent)) = msg.payload.clone() else {
            return Ok(());
        };
        let Some((cap_id, _)) = self.local_cards.iter().next() else {
            tracing::warn!("received REQUEST but have no local capabilities");
            return Ok(());
        };
        let capability_id = cap_id.clone();
        let builder = AclBuilder::new(&self.identity, msg.conversation_id.clone());
        let propose = builder.propose(
            &msg.ontology,
            msg.message_id.clone(),
            Proposal {
                capability_id: capability_id.clone(),
                provider_agent_id: self.identity.peer_id().to_string(),
                quote: "echo".into(),
                expires_at_ns: 0,
            },
        )?;
        let _ = self
            .swarm
            .behaviour_mut()
            .acl
            .send_response(channel, propose);

        self.providers.insert(
            msg.conversation_id.clone(),
            ProviderState {
                requester_peer: peer,
                capability_id,
                intent_payload: if intent.payload.is_empty() {
                    b"(empty)".to_vec()
                } else {
                    intent.payload.to_vec()
                },
            },
        );
        Ok(())
    }

    async fn handle_agree_round2(
        &mut self,
        _peer: PeerId,
        msg: AclMessage,
        channel: request_response::ResponseChannel<AclMessage>,
    ) -> anyhow::Result<()> {
        let Some(prov) = self.providers.remove(&msg.conversation_id) else {
            return Ok(());
        };
        // Execute — for the skeleton, echo the payload back.
        let builder = AclBuilder::new(&self.identity, msg.conversation_id.clone());
        let inform = builder.inform(
            ECHO_ONTOLOGY,
            msg.message_id.clone(),
            TaskResult {
                conversation_id: msg.conversation_id.clone(),
                payload: prov.intent_payload.clone().into(),
                status: ResultStatus::Ok as i32,
                error_message: String::new(),
            },
        )?;
        let _ = self
            .swarm
            .behaviour_mut()
            .acl
            .send_response(channel, inform);

        // Provider's own receipt.
        let receipt_obj = receipt::build(
            &self.identity,
            msg.conversation_id.clone(),
            prov.capability_id,
            prov.requester_peer.to_string(),
            self.identity.peer_id().to_string(),
            ResultStatus::Ok,
        )?;
        self.receipts.insert(&receipt_obj, None).await?;
        metrics::counter!(m::RECEIPTS_TOTAL).increment(1);
        Ok(())
    }

    async fn on_acl_response(
        &mut self,
        peer: PeerId,
        request_id: request_response::OutboundRequestId,
        msg: AclMessage,
    ) -> anyhow::Result<()> {
        if acl_core::verify(&msg).is_err() {
            tracing::warn!(%peer, "acl response signature invalid");
            return Ok(());
        }
        let perf = Performative::try_from(msg.performative).unwrap_or(Performative::Unspecified);

        if let Some(conv) = self.pending_acl_round1.remove(&request_id) {
            if perf != Performative::Propose {
                tracing::warn!(?perf, "expected PROPOSE");
                return Ok(());
            }
            // Round 2: send AGREE.
            let (provider, agree) = {
                let Some(req) = self.requesters.get(&conv) else {
                    return Ok(());
                };
                let Some(provider) = req.provider_peer else {
                    return Ok(());
                };
                let capability_id = req.chosen_capability_id.clone().unwrap_or_default();
                let builder = AclBuilder::new(&self.identity, conv.clone());
                let agree = builder.agree(
                    ECHO_ONTOLOGY,
                    msg.message_id.clone(),
                    Agreement {
                        conversation_id: conv.clone(),
                        capability_id,
                    },
                )?;
                (provider, agree)
            };
            let req_id = self
                .swarm
                .behaviour_mut()
                .acl
                .send_request(&provider, agree);
            self.pending_acl_round2.insert(req_id, conv);
            return Ok(());
        }

        if let Some(conv) = self.pending_acl_round2.remove(&request_id) {
            if perf != Performative::Inform {
                tracing::warn!(?perf, "expected INFORM");
                return Ok(());
            }
            self.complete_intent(peer, &conv, &msg).await?;
        }
        Ok(())
    }

    async fn complete_intent(
        &mut self,
        provider_peer: PeerId,
        conv: &str,
        inform: &AclMessage,
    ) -> anyhow::Result<()> {
        let Some(mut req) = self.requesters.remove(conv) else {
            return Ok(());
        };
        let capability_id = req.chosen_capability_id.clone().unwrap_or_default();

        let receipt_obj = receipt::build(
            &self.identity,
            conv.to_string(),
            capability_id.clone(),
            self.identity.peer_id().to_string(),
            provider_peer.to_string(),
            ResultStatus::Ok,
        )?;
        self.receipts
            .insert(&receipt_obj, Some(inform.signature.as_ref()))
            .await?;
        metrics::counter!(m::RECEIPTS_TOTAL).increment(1);

        let outcome = IntentOutcome {
            conversation_id: conv.to_string(),
            matched_capability_id: capability_id,
            cosine_score: req.chosen_score,
        };
        if let Some(reply) = req.reply.take() {
            let _ = reply.send(Ok(outcome.clone()));
        }
        self.emit(KitedEvent::IntentCompleted(outcome));
        Ok(())
    }
}
