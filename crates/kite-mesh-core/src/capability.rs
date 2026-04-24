//! Capability card construction, signing, and verification.

use kite_mesh_proto::{
    AgentStatus, CANONICAL_EMBEDDING_DIM, CANONICAL_MODEL_ID, CapabilityCard, CapabilityFacets,
    EmbeddingDescriptor,
};
use prost::Message;

use crate::{
    error::{Error, Result},
    facets,
    identity::AgentIdentity,
};

/// Build a signed CapabilityCard.
///
/// Fills in `facet_fingerprint` deterministically and signs the canonical
/// serialization with `identity`.
#[allow(clippy::too_many_arguments)]
pub fn build(
    identity: &AgentIdentity,
    capability_id: String,
    agent_name: String,
    agent_version: String,
    facets: CapabilityFacets,
    description: String,
    vector: Vec<f32>,
    ttl_ns: u64,
) -> Result<CapabilityCard> {
    if vector.len() != CANONICAL_EMBEDDING_DIM as usize {
        return Err(Error::EmbeddingDimension {
            expected: CANONICAL_EMBEDDING_DIM,
            got: vector.len() as u32,
        });
    }

    let now_ns = now_ns();
    let fp = facets::fingerprint(&facets);

    let mut card = CapabilityCard {
        capability_id,
        agent_id: identity.peer_id().to_string(),
        agent_name,
        agent_version,
        ontologies: facets.ontologies.clone(),
        tools: vec![],
        facets: Some(facets),
        description,
        embedding: Some(EmbeddingDescriptor {
            model_id: CANONICAL_MODEL_ID.to_string(),
            dimension: CANONICAL_EMBEDDING_DIM,
            vector,
            normalized: true,
        }),
        pricing: None,
        status: AgentStatus::Available as i32,
        last_seen_ns: now_ns,
        expires_at_ns: now_ns.saturating_add(ttl_ns),
        facet_fingerprint: fp.to_vec().into(),
        endorsements: vec![],
        signature: Default::default(),
        mcp_servers: vec![],
        a2a_agent_card_url: String::new(),
        public_key: identity.public_key().encode_protobuf().into(),
    };

    let bytes = signing_bytes(&card)?;
    let sig = identity.sign(&bytes)?;
    card.signature = sig.into();
    Ok(card)
}

/// Verify a card's signature, fingerprint, and embedding dimension.
pub fn verify(card: &CapabilityCard) -> Result<()> {
    let Some(facets) = card.facets.as_ref() else {
        return Err(Error::InvalidCard("missing facets".into()));
    };
    let Some(embedding) = card.embedding.as_ref() else {
        return Err(Error::InvalidCard("missing embedding".into()));
    };
    if embedding.model_id != CANONICAL_MODEL_ID {
        return Err(Error::UnsupportedModel(embedding.model_id.clone()));
    }
    if embedding.dimension != CANONICAL_EMBEDDING_DIM
        || embedding.vector.len() as u32 != CANONICAL_EMBEDDING_DIM
    {
        return Err(Error::EmbeddingDimension {
            expected: CANONICAL_EMBEDDING_DIM,
            got: embedding.vector.len() as u32,
        });
    }

    let expected_fp = facets::fingerprint(facets);
    if card.facet_fingerprint.as_ref() != expected_fp.as_slice() {
        return Err(Error::FingerprintMismatch);
    }

    let expected_peer: libp2p::PeerId = card
        .agent_id
        .parse()
        .map_err(|_| Error::InvalidCard("bad agent_id".into()))?;
    let public = libp2p::identity::PublicKey::try_decode_protobuf(card.public_key.as_ref())
        .map_err(|e| Error::InvalidCard(format!("bad public key: {e}")))?;
    if libp2p::PeerId::from_public_key(&public) != expected_peer {
        return Err(Error::InvalidCard(
            "public key does not match agent_id".into(),
        ));
    }
    let signing = signing_bytes(card)?;
    AgentIdentity::verify(&public, &signing, card.signature.as_ref())?;
    Ok(())
}

/// Canonical bytes used for signing / verification: the card with an empty
/// `signature` field encoded via prost.
fn signing_bytes(card: &CapabilityCard) -> Result<Vec<u8>> {
    let mut c = card.clone();
    c.signature = Default::default();
    let mut buf = Vec::with_capacity(c.encoded_len());
    c.encode(&mut buf)?;
    Ok(buf)
}

fn now_ns() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_vector() -> Vec<f32> {
        let mut v = vec![0.0f32; CANONICAL_EMBEDDING_DIM as usize];
        v[0] = 1.0;
        v
    }

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

    #[test]
    fn round_trip_sign_and_verify() {
        let id = AgentIdentity::generate();
        let card = build(
            &id,
            "cap-1".into(),
            "python-worker".into(),
            "0.1.0".into(),
            sample_facets(),
            "executes python in a container".into(),
            sample_vector(),
            60_000_000_000,
        )
        .unwrap();
        verify(&card).unwrap();
    }

    #[test]
    fn tampered_description_fails_verification() {
        let id = AgentIdentity::generate();
        let mut card = build(
            &id,
            "cap-1".into(),
            "python-worker".into(),
            "0.1.0".into(),
            sample_facets(),
            "executes python in a container".into(),
            sample_vector(),
            60_000_000_000,
        )
        .unwrap();
        card.description = "executes bash as root".into();
        assert!(matches!(verify(&card), Err(Error::BadSignature)));
    }

    #[test]
    fn tampered_facets_fails_fingerprint_before_signature() {
        let id = AgentIdentity::generate();
        let mut card = build(
            &id,
            "cap-1".into(),
            "python-worker".into(),
            "0.1.0".into(),
            sample_facets(),
            "executes python in a container".into(),
            sample_vector(),
            60_000_000_000,
        )
        .unwrap();
        if let Some(f) = card.facets.as_mut() {
            f.sandbox_level = "none".into();
        }
        assert!(matches!(verify(&card), Err(Error::FingerprintMismatch)));
    }
}
