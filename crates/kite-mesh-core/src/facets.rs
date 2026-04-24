//! Canonical facet fingerprinting.
//!
//! The fingerprint is a blake3 hash over a deterministic serialization of the
//! capability's exact facets. Two cards with identical facets MUST produce the
//! same fingerprint on any implementation. Interop facets
//! (`mcp_servers`, `a2a_agent_card_url`) are deliberately excluded — see
//! MESH_PRD.md §5.

use kite_mesh_proto::{AgentStatus, CapabilityFacets};
use serde::{Deserialize, Serialize};

/// Canonical encoding used for fingerprinting. JSON with sorted keys is used
/// because prost does not guarantee field ordering across versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CanonicalFacets {
    ontologies: Vec<String>,
    tools: Vec<String>,
    sandbox_level: String,
    network_access: String,
    gpu_class: String,
    privacy_tier: String,
    region: String,
    pricing_band: String,
    trust_tier: String,
    status: i32,
}

impl From<&CapabilityFacets> for CanonicalFacets {
    fn from(f: &CapabilityFacets) -> Self {
        let mut tools = f.tools.clone();
        tools.sort();
        tools.dedup();

        let mut ontologies = f.ontologies.clone();
        ontologies.sort();
        ontologies.dedup();

        Self {
            ontologies,
            tools,
            sandbox_level: normalize(&f.sandbox_level),
            network_access: normalize(&f.network_access),
            gpu_class: normalize(&f.gpu_class),
            privacy_tier: normalize(&f.privacy_tier),
            region: normalize(&f.region),
            pricing_band: normalize(&f.pricing_band),
            trust_tier: normalize(&f.trust_tier),
            // status is mutable over a card's lifetime; exclude from fingerprint.
            status: AgentStatus::Unspecified as i32,
        }
    }
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Deterministic 32-byte fingerprint over the canonicalized facets.
pub fn fingerprint(facets: &CapabilityFacets) -> [u8; 32] {
    let canonical = CanonicalFacets::from(facets);
    // serde_json with a stable key order (struct fields are serialized in
    // declaration order, not hash-map order, so this is deterministic).
    let bytes = serde_json::to_vec(&canonical).expect("canonical facets always serialize");
    *blake3::hash(&bytes).as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kite_mesh_proto::AgentStatus;

    fn sample() -> CapabilityFacets {
        CapabilityFacets {
            tools: vec!["python_exec".into(), "bash".into()],
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

    #[test]
    fn order_independence() {
        let mut a = sample();
        let mut b = sample();
        b.tools.reverse();
        a.tools.push("python_exec".into()); // duplicate — should dedupe
        assert_eq!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn case_insensitive_enums() {
        let mut a = sample();
        let mut b = sample();
        a.sandbox_level = "Container".into();
        b.sandbox_level = "CONTAINER".into();
        assert_eq!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn status_does_not_affect_fingerprint() {
        let mut a = sample();
        let mut b = sample();
        a.status = AgentStatus::Available as i32;
        b.status = AgentStatus::Busy as i32;
        assert_eq!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn distinct_facets_distinct_fingerprint() {
        let a = sample();
        let mut b = sample();
        b.sandbox_level = "none".into();
        assert_ne!(fingerprint(&a), fingerprint(&b));
    }
}
