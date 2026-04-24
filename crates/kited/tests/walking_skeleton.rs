//! End-to-end walking skeleton test.
//!
//! Spawns two daemons in-process, waits for them to mDNS-discover each other,
//! has one publish a capability, has the other submit an intent, and verifies
//! that the full ACL round-trip lands a signed Receipt on both sides.

use std::time::Duration;

use kite_mesh_core::{capability, intent};
use kite_mesh_discovery::CanonicalEncoder;
use kite_mesh_proto::{AgentStatus, CapabilityFacets};
use kited::{Kited, KitedHandle, config::KitedConfig, daemon::KitedEvent, metrics};
use tokio::time::{Instant, timeout};

const TEST_TIMEOUT: Duration = Duration::from_secs(120);
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);

fn sample_facets() -> CapabilityFacets {
    CapabilityFacets {
        tools: vec!["python_exec".into()],
        ontologies: vec!["code-execution".into()],
        sandbox_level: "container".into(),
        network_access: "egress-limited".into(),
        gpu_class: "none".into(),
        privacy_tier: "trusted".into(),
        region: "local".into(),
        pricing_band: "free".into(),
        trust_tier: "local".into(),
        status: AgentStatus::Available as i32,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_agents_discover_and_complete_intent() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_test_writer()
        .try_init()
        .ok();

    // One global Prometheus recorder is fine in-process; both daemons publish
    // to the same handle. For production separate processes each install their
    // own recorder naturally.
    let prom = metrics::install().expect("install metrics");

    let scenario = async {
        let encoder = CanonicalEncoder::new().await.expect("encoder boot");

        let tmp_a = tempfile::tempdir().expect("tmp a");
        let tmp_b = tempfile::tempdir().expect("tmp b");
        let cfg_a = KitedConfig {
            identity: Default::default(),
            network: kited::config::NetworkConfig {
                listen_addr: "/ip4/0.0.0.0/tcp/0".into(),
                enable_mdns: true,
            },
            storage: kited::config::StorageConfig {
                data_dir: tmp_a.path().to_path_buf(),
            },
            metrics: Default::default(),
        };
        let cfg_b = KitedConfig {
            identity: Default::default(),
            network: kited::config::NetworkConfig {
                listen_addr: "/ip4/0.0.0.0/tcp/0".into(),
                enable_mdns: true,
            },
            storage: kited::config::StorageConfig {
                data_dir: tmp_b.path().to_path_buf(),
            },
            metrics: Default::default(),
        };

        let mut a = Kited::start(cfg_a, prom.clone()).await.expect("kited a");
        let mut b = Kited::start(cfg_b, prom.clone()).await.expect("kited b");

        wait_for_peer(&mut a, b.peer_id).await;
        wait_for_peer(&mut b, a.peer_id).await;

        // A publishes a capability signed with its own identity.
        let description_a =
            "executes python in a sandboxed linux container with a 60 second timeout";
        let vector_a = encoder.encode(description_a).await.expect("encode a");
        let card = capability::build(
            &a.identity,
            "cap-python-1".to_string(),
            "python-worker".to_string(),
            "0.1.0".to_string(),
            sample_facets(),
            description_a.to_string(),
            vector_a,
            300_000_000_000,
        )
        .expect("build card");

        a.inbox
            .publish_capability(card.clone())
            .await
            .expect("publish");
        wait_for_published(&mut a).await;

        // B submits an intent.
        let description_b = "run python code inside a container sandbox";
        let vector_b = encoder.encode(description_b).await.expect("encode b");
        let built = intent::build(intent::IntentInput {
            requester_agent_id: b.peer_id.to_string(),
            facets: sample_facets(),
            description: description_b.to_string(),
            embedding: vector_b.clone(),
            payload: b"hello from b".to_vec(),
            deadline_ns: 0,
        })
        .expect("intent");

        let outcome = b
            .inbox
            .submit_intent(sample_facets(), vector_b, built.intent)
            .await
            .expect("intent completed");

        assert_eq!(outcome.matched_capability_id, card.capability_id);
        assert!(
            outcome.cosine_score > 0.5,
            "cosine score too low: {}",
            outcome.cosine_score
        );

        // Receipts on both sides.
        let stored_a = a
            .receipts
            .get(&outcome.conversation_id)
            .await
            .expect("store a")
            .expect("row a");
        let stored_b = b
            .receipts
            .get(&outcome.conversation_id)
            .await
            .expect("store b")
            .expect("row b");
        kite_mesh_core::receipt::verify(&stored_a.receipt).expect("verify a");
        kite_mesh_core::receipt::verify(&stored_b.receipt).expect("verify b");
        assert_eq!(stored_a.receipt.capability_id, card.capability_id);
        assert_eq!(stored_b.receipt.capability_id, card.capability_id);

        // Metrics advanced.
        let dump = prom.render();
        assert!(dump.contains("kite_mesh_directory_records_total"));
        assert!(dump.contains("kite_mesh_receipts_total"));
        assert!(dump.contains("kite_mesh_match_accept_total"));

        a.inbox.shutdown().await;
        b.inbox.shutdown().await;
    };

    timeout(TEST_TIMEOUT, scenario)
        .await
        .expect("walking skeleton timed out");
}

async fn wait_for_peer(h: &mut KitedHandle, peer: libp2p::PeerId) {
    let deadline = Instant::now() + DISCOVERY_TIMEOUT;
    while Instant::now() < deadline {
        let Some(evt) = timeout(Duration::from_secs(5), h.events.recv())
            .await
            .ok()
            .flatten()
        else {
            continue;
        };
        if let KitedEvent::PeerDiscovered(p) = evt
            && p == peer
        {
            return;
        }
    }
    panic!("timed out waiting to discover {peer}");
}

async fn wait_for_published(h: &mut KitedHandle) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        let Some(evt) = timeout(Duration::from_secs(5), h.events.recv())
            .await
            .ok()
            .flatten()
        else {
            continue;
        };
        if let KitedEvent::CapabilityPublished { .. } = evt {
            return;
        }
    }
    panic!("timed out waiting for publish confirmation");
}
