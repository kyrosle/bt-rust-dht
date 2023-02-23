use std::convert::TryInto;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::id::NODE_ID_LEN;

const SOCKET_ADDR_V4_LEN: usize = 6;
const SOCKET_ADDR_V6_LEN: usize = 18;

pub mod values {
  use std::net::SocketAddr;

  use serde::{
    de::{Error as _, Visitor},
    ser::SerializeSeq,
    Deserializer, Serializer,
  };
  use serde_bytes::{ByteBuf, Bytes};

  pub fn serialize<S>(addrs: &[SocketAddr], s: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut seq = s.serialize_seq(Some(addrs.len()))?;
    for addr in addrs {
      seq.serialize_element(Bytes::new(&super::encode_socket_addr(addr)))?;
    }
    seq.end()
  }

  pub fn deserialize<'de, D>(d: D) -> Result<Vec<SocketAddr>, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct SocketAddrsVisitor;

    impl<'de> Visitor<'de> for SocketAddrsVisitor {
      type Value = Vec<SocketAddr>;

      fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "list of byte strings")
      }

      fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
      where
        A: serde::de::SeqAccess<'de>,
      {
        let mut output = Vec::with_capacity(seq.size_hint().unwrap_or(0));

        while let Some(bytes) = seq.next_element::<ByteBuf>()? {
          let item = super::decode_socket_addr(&bytes)
            .ok_or_else(|| A::Error::invalid_length(bytes.len(), &self))?;
          output.push(item);
        }

        Ok(output)
      }
    }
    d.deserialize_seq(SocketAddrsVisitor)
  }
}

fn decode_socket_addr(src: &[u8]) -> Option<SocketAddr> {
  if src.len() == SOCKET_ADDR_V4_LEN {
    let addr: [u8; 4] = src.get(..4)?.try_into().ok()?;
    let addr = Ipv4Addr::from(addr);
    let port = u16::from_be_bytes(src.get(4..)?.try_into().ok()?);
    Some((addr, port).into())
  } else if src.len() == SOCKET_ADDR_V6_LEN {
    let addr: [u8; 16] = src.get(..16)?.try_into().ok()?;
    let addr = Ipv6Addr::from(addr);
    let port = u16::from_be_bytes(src.get(16..)?.try_into().ok()?);
    Some((addr, port).into())
  } else {
    None
  }
}

// TODO: Should returning `ArrayVec` to avoid lot of small allocations.
fn encode_socket_addr(addr: &SocketAddr) -> Vec<u8> {
  let mut buffer = match addr {
    SocketAddr::V4(addr) => {
      let mut buffer = Vec::with_capacity(6);
      buffer.extend(addr.ip().octets().as_ref());
      buffer
    }
    SocketAddr::V6(addr) => {
      let mut buffer = Vec::with_capacity(18);
      buffer.extend(addr.ip().octets().as_ref());
      buffer
    }
  };

  buffer.extend(addr.port().to_be_bytes().as_ref());
  buffer
}

mod nodes {
  use serde::{
    de::Error as _, ser::Error as _, Deserialize, Deserializer, Serializer,
  };
  use serde_bytes::ByteBuf;

  use crate::{id::NodeId, node::NodeHandle};

  pub fn serialize<S, const ADDR_LEN: usize>(
    nodes: &[NodeHandle],
    s: S,
  ) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut buffer =
      Vec::with_capacity(nodes.len() * super::NODE_ID_LEN + ADDR_LEN);

    for node in nodes {
      let encoded_addr = super::encode_socket_addr(&node.addr);

      if encoded_addr.len() != ADDR_LEN {
        return Err(S::Error::custom("unexpected address family"));
      }

      buffer.extend(node.id.as_ref());
      buffer.extend(encoded_addr);
    }
    s.serialize_bytes(&buffer)
  }

  pub fn deserialize<'de, D, const ADDR_LEN: usize>(
    d: D,
  ) -> Result<Vec<NodeHandle>, D::Error>
  where
    D: Deserializer<'de>,
  {
    let buffer = ByteBuf::deserialize(d)?;
    let chunks = buffer.chunks_exact(super::NODE_ID_LEN + ADDR_LEN);

    if !chunks.remainder().is_empty() {
      let msg = format!("multiple of {}", (super::NODE_ID_LEN + ADDR_LEN));
      return Err(D::Error::invalid_length(buffer.len(), &msg.as_ref()));
    }

    let nodes = chunks
      .filter_map(|chunk| {
        let id = NodeId::try_from(&chunk[..super::NODE_ID_LEN]).ok()?;
        let addr = super::decode_socket_addr(&chunk[super::NODE_ID_LEN..])?;

        Some(NodeHandle { id, addr })
      })
      .collect();

    Ok(nodes)
  }
}

/// Serialize/deserialize `Vec` of `NodeHandle` in compact format.
/// Specialized for ipv4 addresses.
pub mod nodes_v4 {
  use serde::{Deserializer, Serializer};

  use crate::node::NodeHandle;

  pub fn serialize<S>(nodes: &[NodeHandle], s: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    super::nodes::serialize::<S, { super::SOCKET_ADDR_V4_LEN }>(nodes, s)
  }

  pub fn deserialize<'de, D>(d: D) -> Result<Vec<NodeHandle>, D::Error>
  where
    D: Deserializer<'de>,
  {
    super::nodes::deserialize::<D, { super::SOCKET_ADDR_V4_LEN }>(d)
  }
}

/// Serialize/deserialize `Vec` of `NodeHandle` in compact format.
/// Specialized for ipv6 addresses.
pub mod nodes_v6 {
  use serde::{Deserializer, Serializer};

  use crate::node::NodeHandle;

  pub fn serialize<S>(nodes: &[NodeHandle], s: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    super::nodes::serialize::<S, { super::SOCKET_ADDR_V6_LEN }>(nodes, s)
  }

  pub fn deserialize<'de, D>(d: D) -> Result<Vec<NodeHandle>, D::Error>
  where
    D: Deserializer<'de>,
  {
    super::nodes::deserialize::<D, { super::SOCKET_ADDR_V6_LEN }>(d)
  }
}
