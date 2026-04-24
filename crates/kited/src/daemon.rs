//! Kited daemon — the in-process entry point used by both `main.rs` and the
//! integration test.

use std::sync::Arc;

use kite_mesh_core::{AgentIdentity, receipt::ReceiptStore};
use kite_mesh_discovery::{
    CanonicalEncoder, LshParams,
    lsh::{AngularLsh, default_seed},
};
use kite_mesh_transport::build_swarm;
use libp2p::PeerId;
use metrics_exporter_prometheus::PrometheusHandle;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use crate::{config::KitedConfig, event_loop::EventLoop};

/// Commands accepted by the running daemon over the inbox channel.
pub enum Command {
    PublishCapability {
        card: Box<kite_mesh_proto::CapabilityCard>,
        ack: oneshot::Sender<anyhow::Result<()>>,
    },
    SubmitIntent {
        required: Box<kite_mesh_proto::CapabilityFacets>,
        query_vector: Vec<f32>,
        intent: Box<kite_mesh_proto::Intent>,
        reply: oneshot::Sender<anyhow::Result<IntentOutcome>>,
    },
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct IntentOutcome {
    pub conversation_id: String,
    pub matched_capability_id: String,
    pub cosine_score: f32,
}

/// Observable events emitted by the daemon for test assertions.
#[derive(Debug, Clone)]
pub enum KitedEvent {
    Listening(String),
    PeerDiscovered(PeerId),
    CapabilityPublished {
        capability_id: String,
        record_count: u32,
    },
    IntentCompleted(IntentOutcome),
}

/// Inbox handle given to callers.
#[derive(Clone)]
pub struct Inbox {
    tx: mpsc::Sender<Command>,
}

impl Inbox {
    pub async fn publish_capability(
        &self,
        card: kite_mesh_proto::CapabilityCard,
    ) -> anyhow::Result<()> {
        let (ack, rx) = oneshot::channel();
        self.tx
            .send(Command::PublishCapability {
                card: Box::new(card),
                ack,
            })
            .await?;
        rx.await?
    }

    pub async fn submit_intent(
        &self,
        required: kite_mesh_proto::CapabilityFacets,
        query_vector: Vec<f32>,
        intent: kite_mesh_proto::Intent,
    ) -> anyhow::Result<IntentOutcome> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Command::SubmitIntent {
                required: Box::new(required),
                query_vector,
                intent: Box::new(intent),
                reply,
            })
            .await?;
        rx.await?
    }

    pub async fn shutdown(&self) {
        let _ = self.tx.send(Command::Shutdown).await;
    }
}

/// Handle to a running daemon — carries the inbox, event stream, metrics, and
/// background task handle.
pub struct KitedHandle {
    pub inbox: Inbox,
    pub peer_id: PeerId,
    /// Clone of the daemon's Ed25519 identity — exposed so test harnesses and
    /// SDK callers can sign CapabilityCards against the running daemon.
    pub identity: AgentIdentity,
    pub events: mpsc::Receiver<KitedEvent>,
    pub metrics: PrometheusHandle,
    pub receipts: ReceiptStore,
    join: JoinHandle<anyhow::Result<()>>,
}

impl KitedHandle {
    pub async fn join(self) -> anyhow::Result<()> {
        self.join.await?
    }
}

pub struct Kited;

impl Kited {
    /// Boot a daemon in-process. Spawns the libp2p event loop on a Tokio task.
    pub async fn start(
        config: KitedConfig,
        metrics: PrometheusHandle,
    ) -> anyhow::Result<KitedHandle> {
        let identity = match &config.identity.keypair_path {
            Some(path) if path.exists() => AgentIdentity::load(path).await?,
            _ => AgentIdentity::generate(),
        };
        let peer_id = identity.peer_id();

        let encoder = CanonicalEncoder::new().await?;
        let lsh = Arc::new(AngularLsh::new(
            LshParams::default_for_skeleton(),
            &default_seed(),
        ));
        let receipts = ReceiptStore::open(&config.storage.data_dir.join("receipts.sqlite")).await?;

        let swarm = build_swarm(identity.keypair().clone())?;

        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(32);
        let (evt_tx, evt_rx) = mpsc::channel::<KitedEvent>(32);

        let identity_for_loop = identity.clone();
        let loop_handle = EventLoop::spawn(
            swarm,
            identity_for_loop,
            encoder,
            lsh,
            receipts.clone(),
            config,
            cmd_rx,
            evt_tx,
        )
        .await?;

        Ok(KitedHandle {
            inbox: Inbox { tx: cmd_tx },
            peer_id,
            identity,
            events: evt_rx,
            metrics,
            receipts,
            join: loop_handle,
        })
    }
}
