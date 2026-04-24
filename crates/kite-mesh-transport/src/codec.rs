//! Wire codecs for request-response exchanges.
//!
//! - `AclCodec` carries `AclMessage` payloads for direct ACL negotiation.
//! - `CardCodec` serves capability card fetches after a DHT DirectoryRecord hit.

use std::io;

use async_trait::async_trait;
use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use kite_mesh_proto::{AclMessage, CapabilityCard};
use libp2p::{StreamProtocol, request_response};
use prost::Message;

const MAX_MESSAGE_BYTES: usize = 1 << 20; // 1 MiB hard cap — skeleton-level sanity bound

pub const ACL_PROTOCOL: StreamProtocol = StreamProtocol::new("/kite-mesh/acl/1.0.0");
pub const CARD_PROTOCOL: StreamProtocol = StreamProtocol::new("/kite-mesh/card/1.0.0");

/// Request-response codec carrying Kite ACL envelopes.
#[derive(Clone, Default)]
pub struct AclCodec;

#[async_trait]
impl request_response::Codec for AclCodec {
    type Protocol = StreamProtocol;
    type Request = AclMessage;
    type Response = AclMessage;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_framed(io).await
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_framed(io).await
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_framed(io, &req).await
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_framed(io, &res).await
    }
}

/// Request-response codec for CapabilityCard fetches.
#[derive(Clone, Default)]
pub struct CardCodec;

#[async_trait]
impl request_response::Codec for CardCodec {
    type Protocol = StreamProtocol;
    type Request = CardRequest;
    type Response = CardResponse;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let body: CardRequestProto = read_framed(io).await?;
        Ok(CardRequest {
            capability_id: body.capability_id,
        })
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let body: CardResponseProto = read_framed(io).await?;
        Ok(match body.card {
            Some(c) => CardResponse::Found(c),
            None => CardResponse::NotFound,
        })
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_framed(
            io,
            &CardRequestProto {
                capability_id: req.capability_id,
            },
        )
        .await
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let card = match res {
            CardResponse::Found(c) => Some(c),
            CardResponse::NotFound => None,
        };
        write_framed(io, &CardResponseProto { card }).await
    }
}

#[derive(Debug, Clone)]
pub struct CardRequest {
    pub capability_id: String,
}

#[derive(Debug, Clone)]
pub enum CardResponse {
    Found(CapabilityCard),
    NotFound,
}

#[derive(Clone, prost::Message)]
struct CardRequestProto {
    #[prost(string, tag = "1")]
    capability_id: String,
}

#[derive(Clone, prost::Message)]
struct CardResponseProto {
    #[prost(message, optional, tag = "1")]
    card: Option<CapabilityCard>,
}

/// Read a length-prefixed prost message.
async fn read_framed<T, M>(io: &mut T) -> io::Result<M>
where
    T: AsyncRead + Unpin + Send,
    M: Message + Default,
{
    let mut len_buf = [0u8; 4];
    io.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message too large",
        ));
    }
    let mut buf = vec![0u8; len];
    io.read_exact(&mut buf).await?;
    M::decode(buf.as_slice()).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

async fn write_framed<T, M>(io: &mut T, msg: &M) -> io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
    M: Message,
{
    let len = msg.encoded_len();
    if len > MAX_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message too large",
        ));
    }
    let mut buf = Vec::with_capacity(4 + len);
    buf.extend_from_slice(&(len as u32).to_be_bytes());
    msg.encode(&mut buf).map_err(io::Error::other)?;
    io.write_all(&buf).await?;
    io.flush().await?;
    Ok(())
}
