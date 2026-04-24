//! Intent construction.
//!
//! Intents carry the full task description; in Phase 1 they are paired with
//! compact `IntentAdvert`s broadcast over GossipSub. The walking skeleton
//! uses pull-only discovery so the advert path is not exercised.

use kite_mesh_proto::{CANONICAL_EMBEDDING_DIM, CapabilityFacets, Intent};

use crate::error::{Error, Result};

pub struct IntentInput {
    pub requester_agent_id: String,
    pub facets: CapabilityFacets,
    pub description: String,
    pub embedding: Vec<f32>,
    pub payload: Vec<u8>,
    pub deadline_ns: u64,
}

pub struct BuiltIntent {
    pub intent: Intent,
    pub facets: CapabilityFacets,
    pub embedding: Vec<f32>,
}

pub fn build(input: IntentInput) -> Result<BuiltIntent> {
    if input.embedding.len() != CANONICAL_EMBEDDING_DIM as usize {
        return Err(Error::EmbeddingDimension {
            expected: CANONICAL_EMBEDDING_DIM,
            got: input.embedding.len() as u32,
        });
    }
    let intent = Intent {
        intent_id: uuid::Uuid::new_v4().to_string(),
        requester_agent_id: input.requester_agent_id,
        description: input.description,
        payload: input.payload.into(),
        deadline_ns: input.deadline_ns,
    };
    Ok(BuiltIntent {
        intent,
        facets: input.facets,
        embedding: input.embedding,
    })
}
