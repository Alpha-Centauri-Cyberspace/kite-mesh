//! ACL envelope construction and signature verification.

use kite_mesh_proto::{
    AclMessage, Agreement, Intent, Performative, Proposal, Receipt, TaskResult, TrustContext,
    acl_message::Payload,
};
use prost::Message;

use crate::{error::Result, identity::AgentIdentity};

/// Builder for signed ACL envelopes.
pub struct AclBuilder<'a> {
    identity: &'a AgentIdentity,
    conversation_id: String,
}

impl<'a> AclBuilder<'a> {
    pub fn new(identity: &'a AgentIdentity, conversation_id: impl Into<String>) -> Self {
        Self {
            identity,
            conversation_id: conversation_id.into(),
        }
    }

    pub fn request(&self, ontology: impl Into<String>, intent: Intent) -> Result<AclMessage> {
        self.build(
            Performative::Request,
            ontology.into(),
            "",
            Some(Payload::Intent(intent)),
        )
    }

    pub fn propose(
        &self,
        ontology: impl Into<String>,
        in_reply_to: impl Into<String>,
        proposal: Proposal,
    ) -> Result<AclMessage> {
        self.build(
            Performative::Propose,
            ontology.into(),
            &in_reply_to.into(),
            Some(Payload::Proposal(proposal)),
        )
    }

    pub fn agree(
        &self,
        ontology: impl Into<String>,
        in_reply_to: impl Into<String>,
        agreement: Agreement,
    ) -> Result<AclMessage> {
        self.build(
            Performative::Agree,
            ontology.into(),
            &in_reply_to.into(),
            Some(Payload::Agreement(agreement)),
        )
    }

    pub fn inform(
        &self,
        ontology: impl Into<String>,
        in_reply_to: impl Into<String>,
        result: TaskResult,
    ) -> Result<AclMessage> {
        self.build(
            Performative::Inform,
            ontology.into(),
            &in_reply_to.into(),
            Some(Payload::Result(result)),
        )
    }

    pub fn receipt(
        &self,
        ontology: impl Into<String>,
        in_reply_to: impl Into<String>,
        receipt: Receipt,
    ) -> Result<AclMessage> {
        self.build(
            Performative::Inform,
            ontology.into(),
            &in_reply_to.into(),
            Some(Payload::Receipt(receipt)),
        )
    }

    fn build(
        &self,
        performative: Performative,
        ontology: String,
        in_reply_to: &str,
        payload: Option<Payload>,
    ) -> Result<AclMessage> {
        let mut msg = AclMessage {
            sender_id: self.identity.peer_id().to_string(),
            signature: Default::default(),
            timestamp_ns: now_ns(),
            message_id: uuid::Uuid::new_v4().to_string(),
            performative: performative as i32,
            ontology,
            conversation_id: self.conversation_id.clone(),
            in_reply_to: in_reply_to.to_string(),
            payload,
            trust: Some(TrustContext {
                tier: "local".into(),
                endorser_peer_ids: vec![],
            }),
            sender_public_key: self.identity.public_key().encode_protobuf().into(),
        };
        let bytes = signing_bytes(&msg)?;
        let sig = self.identity.sign(&bytes)?;
        msg.signature = sig.into();
        Ok(msg)
    }
}

/// Verify an ACL envelope against the sender's bundled public key.
pub fn verify(msg: &AclMessage) -> Result<()> {
    let expected_peer: libp2p::PeerId = msg
        .sender_id
        .parse()
        .map_err(|_| crate::Error::InvalidCard("bad sender peer id".into()))?;
    let public = libp2p::identity::PublicKey::try_decode_protobuf(msg.sender_public_key.as_ref())
        .map_err(|e| crate::Error::InvalidCard(format!("bad sender public key: {e}")))?;
    if libp2p::PeerId::from_public_key(&public) != expected_peer {
        return Err(crate::Error::InvalidCard(
            "sender public key does not match sender_id".into(),
        ));
    }
    let bytes = signing_bytes(msg)?;
    AgentIdentity::verify(&public, &bytes, msg.signature.as_ref())
}

fn signing_bytes(msg: &AclMessage) -> Result<Vec<u8>> {
    let mut m = msg.clone();
    m.signature = Default::default();
    let mut buf = Vec::with_capacity(m.encoded_len());
    m.encode(&mut buf)?;
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

    #[test]
    fn round_trip_sign_and_verify() {
        let id = AgentIdentity::generate();
        let builder = AclBuilder::new(&id, "conv-1");
        let intent = Intent {
            intent_id: "int-1".into(),
            requester_agent_id: id.peer_id().to_string(),
            description: "please echo hello".into(),
            payload: bytes::Bytes::from_static(b"hello"),
            deadline_ns: 0,
        };
        let msg = builder.request("code-execution", intent).unwrap();
        verify(&msg).unwrap();
    }

    #[test]
    fn tamper_fails_verification() {
        let id = AgentIdentity::generate();
        let builder = AclBuilder::new(&id, "conv-1");
        let mut msg = builder
            .request(
                "code-execution",
                Intent {
                    intent_id: "int-1".into(),
                    requester_agent_id: id.peer_id().to_string(),
                    description: "hi".into(),
                    payload: Default::default(),
                    deadline_ns: 0,
                },
            )
            .unwrap();
        msg.ontology = "tampered".into();
        assert!(verify(&msg).is_err());
    }
}
