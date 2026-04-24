//! Directory-record key derivation and record construction.

use blake3::Hasher;
use kite_mesh_core::identity::AgentIdentity;
use kite_mesh_proto::{CANONICAL_MODEL_ID, CapabilityCard, DirectoryRecord, PROTOCOL_VERSION};
use prost::Message;

use crate::lsh::{AngularLsh, LshParams};

/// Derive the exact DHT key for table `table_id` with hash code `hash_code`.
///
/// K_i = blake3(protocol_version || facet_fingerprint || model_id || i || h_i)
pub fn dht_key(
    facet_fingerprint: &[u8],
    model_id: &str,
    table_id: u32,
    hash_code: &[u8],
) -> Vec<u8> {
    let mut h = Hasher::new();
    h.update(&PROTOCOL_VERSION.to_le_bytes());
    h.update(facet_fingerprint);
    h.update(model_id.as_bytes());
    h.update(&table_id.to_le_bytes());
    h.update(hash_code);
    h.finalize().as_bytes().to_vec()
}

pub struct DirectoryPublication {
    pub key: Vec<u8>,
    pub record: DirectoryRecord,
    pub record_bytes: Vec<u8>,
}

/// Build `L` signed DirectoryRecords for a capability card — one per LSH table.
pub fn build_records(
    card: &CapabilityCard,
    lsh: &AngularLsh,
    identity: &AgentIdentity,
    ttl_ns: u64,
) -> kite_mesh_core::Result<Vec<DirectoryPublication>> {
    let embedding = card
        .embedding
        .as_ref()
        .ok_or_else(|| kite_mesh_core::Error::InvalidCard("missing embedding".into()))?;
    let signatures = lsh.signatures(&embedding.vector);
    let status = match card.status {
        x if x == kite_mesh_proto::AgentStatus::Available as i32 => "available",
        x if x == kite_mesh_proto::AgentStatus::Busy as i32 => "busy",
        x if x == kite_mesh_proto::AgentStatus::Draining as i32 => "draining",
        _ => "offline",
    }
    .to_string();

    let mut out = Vec::with_capacity(signatures.len());
    let publisher_pk_bytes = identity.public_key().encode_protobuf();
    for (table_id, sig) in signatures.into_iter().enumerate() {
        let key = dht_key(
            card.facet_fingerprint.as_ref(),
            CANONICAL_MODEL_ID,
            table_id as u32,
            &sig,
        );
        let mut record = DirectoryRecord {
            capability_id: card.capability_id.clone(),
            agent_id: card.agent_id.clone(),
            peer_id: identity.peer_id().to_string(),
            facet_fingerprint: card.facet_fingerprint.clone(),
            model_id: CANONICAL_MODEL_ID.to_string(),
            table_id: table_id as u32,
            hash_code: sig.into(),
            status: status.clone(),
            ttl_ns,
            signature: Default::default(),
            protocol_version: PROTOCOL_VERSION,
            publisher_public_key: publisher_pk_bytes.clone().into(),
        };
        let signing = signing_bytes(&record);
        record.signature = identity.sign(&signing)?.into();
        let mut record_bytes = Vec::with_capacity(record.encoded_len());
        record.encode(&mut record_bytes)?;
        out.push(DirectoryPublication {
            key,
            record,
            record_bytes,
        });
    }
    Ok(out)
}

/// Keys to look up for an intent with the given facet fingerprint + query embedding.
pub fn lookup_keys(
    facet_fingerprint: &[u8],
    query_vector: &[f32],
    lsh: &AngularLsh,
) -> Vec<(u32, Vec<u8>)> {
    lsh.signatures(query_vector)
        .into_iter()
        .enumerate()
        .map(|(table_id, sig)| {
            (
                table_id as u32,
                dht_key(facet_fingerprint, CANONICAL_MODEL_ID, table_id as u32, &sig),
            )
        })
        .collect()
}

/// Verify a signed DirectoryRecord against the publisher's bundled public key.
pub fn verify_record(record: &DirectoryRecord) -> kite_mesh_core::Result<()> {
    let expected_peer: libp2p::PeerId = record
        .peer_id
        .parse()
        .map_err(|_| kite_mesh_core::Error::InvalidCard("bad peer id in record".into()))?;
    let public =
        libp2p::identity::PublicKey::try_decode_protobuf(record.publisher_public_key.as_ref())
            .map_err(|e| kite_mesh_core::Error::InvalidCard(format!("bad publisher key: {e}")))?;
    if libp2p::PeerId::from_public_key(&public) != expected_peer {
        return Err(kite_mesh_core::Error::InvalidCard(
            "publisher public key does not match peer_id".into(),
        ));
    }
    let signing = signing_bytes(record);
    AgentIdentity::verify(&public, &signing, record.signature.as_ref())
}

fn signing_bytes(record: &DirectoryRecord) -> Vec<u8> {
    let mut r = record.clone();
    r.signature = Default::default();
    let mut buf = Vec::with_capacity(r.encoded_len());
    r.encode(&mut buf).expect("DirectoryRecord encodes");
    buf
}

pub const DEFAULT_LSH_PARAMS: LshParams = LshParams {
    k: crate::lsh::DEFAULT_K,
    l: crate::lsh::DEFAULT_L,
    dim: kite_mesh_proto::CANONICAL_EMBEDDING_DIM,
};
