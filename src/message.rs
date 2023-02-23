use crate::compact;
use std::{fmt, net::SocketAddr};

use serde::{
  de::{Error as _, IgnoredAny, Visitor},
  ser::SerializeSeq,
  Deserialize, Serialize,
};

use crate::{
  id::{InfoHash, NodeId},
  node::NodeHandle,
};

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Message {
  #[serde(rename = "t", with = "serde_bytes")]
  pub transaction_id: Vec<u8>,
  #[serde(flatten)]
  pub body: MessageBody,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
#[serde(tag = "y")]
pub enum MessageBody {
  #[serde(rename = "q")]
  Request(Request),
  #[serde(rename = "r", with = "unflatten::response")]
  Response(Response),
  #[serde(rename = "e", with = "unflatten::error")]
  Error(Error),
}

mod unflatten {
  macro_rules! impl_unflatten {
    ($mod:ident, $field:literal) => {
      pub mod $mod {
        use serde::{Deserialize, Deserializer, Serialize, Serializer};

        #[derive(Serialize, Deserialize)]
        struct Wrapper<T> {
          #[serde(rename = $field)]
          field: T,
        }

        pub(crate) fn serialize<T: Serialize, S: Serializer>(
          value: &T,
          s: S,
        ) -> Result<S::Ok, S::Error> {
          Wrapper { field: value }.serialize(s)
        }

        pub(crate) fn deserialize<
          'de,
          T: Deserialize<'de>,
          D: Deserializer<'de>,
        >(
          d: D,
        ) -> Result<T, D::Error> {
          let wrapper = Wrapper::deserialize(d)?;
          Ok(wrapper.field)
        }
      }
    };
  }

  impl_unflatten!(response, "r");
  impl_unflatten!(error, "e");
}

// TODO:
// unrecognized requests which contain either
// an 'info_hash' or 'target' arguments should be
// interpreted as 'find_node' as per Mainline DHT extensions.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
#[serde(tag = "q", content = "a")]
#[serde(rename_all = "snake_case")]
pub enum Request {
  Ping(PingRequest),
  FindNode(FindNodeRequest),
  GetPeers(GetPeersRequest),
  AnnouncePeer(AnnouncePeerRequest),
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct PingRequest {
  pub id: NodeId,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct FindNodeRequest {
  pub id: NodeId,
  pub target: NodeId,

  #[serde(with = "want", default, skip_serializing_if = "Option::is_none")]
  pub want: Option<Want>,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct GetPeersRequest {
  pub id: NodeId,
  pub info_hash: InfoHash,

  #[serde(with = "want", default, skip_serializing_if = "Option::is_none")]
  pub want: Option<Want>,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct AnnouncePeerRequest {
  pub id: NodeId,
  pub info_hash: InfoHash,
  pub port: Option<u16>,
  #[serde(with = "serde_bytes")]
  pub token: Vec<u8>,
}

#[derive(Clone, Eq, PartialEq, Debug, Copy)]
pub enum Want {
  // The peer wants only ipv4 contacts
  V4,
  // The peer wants only ipv6 contacts
  V6,
  // The peer wants both ipv4 and ipv6 contacts
  Both,
}

/// Helper to serialize or deserialize the enum `Want`
mod want {
  use serde::{de::Visitor, ser::SerializeSeq, Deserializer, Serializer};
  use serde_bytes::Bytes;

  use super::Want;

  pub fn serialize<S: Serializer>(
    want: &Option<Want>,
    s: S,
  ) -> Result<S::Ok, S::Error> {
    let len = match want {
      None => 0,
      Some(Want::V4 | Want::V6) => 1,
      Some(Want::Both) => 2,
    };

    let mut seq = s.serialize_seq(Some(len))?;

    if matches!(want, Some(Want::V4 | Want::Both)) {
      seq.serialize_element(Bytes::new(b"n4"))?;
    }

    if matches!(want, Some(Want::V6 | Want::Both)) {
      seq.serialize_element(Bytes::new(b"n6"))?;
    }

    seq.end()
  }

  pub fn deserialize<'de, D>(d: D) -> Result<Option<Want>, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct WantVisitor;

    impl<'de> Visitor<'de> for WantVisitor {
      type Value = Option<Want>;

      fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "a list of strings")
      }

      fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
      where
        A: serde::de::SeqAccess<'de>,
      {
        let mut value = None;

        while let Some(s) = seq.next_element::<String>()? {
          value = match (value, s.as_str().trim()) {
            (None, "n4" | "N4") => Some(Want::V4),
            (None, "n6" | "N6") => Some(Want::V6),
            (Some(Want::V4), "n6" | "N6") => Some(Want::Both),
            (Some(Want::V6), "n4" | "N4") => Some(Want::Both),
            (_, _) => value,
          }
        }

        Ok(value)
      }
    }

    d.deserialize_seq(WantVisitor)
  }
}

/// Helper to serialize or deserialize the `port` field.
mod port {
  use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};

  #[derive(Serialize, Deserialize)]
  struct Wrapper {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    port: Option<u16>,

    #[serde(
      default,
      skip_serializing_if = "is_false",
      deserialize_with = "deserialize_bool"
    )]
    implied_port: bool,
  }

  pub fn serialize<S: Serializer>(
    port: &Option<u16>,
    s: S,
  ) -> Result<S::Ok, S::Error> {
    Wrapper {
      implied_port: port.is_none(),
      port: *port,
    }
    .serialize(s)
  }

  pub fn deserialize<'de, D: Deserializer<'de>>(
    d: D,
  ) -> Result<Option<u16>, D::Error> {
    let wrapper = Wrapper::deserialize(d)?;

    if wrapper.implied_port {
      Ok(None)
    } else if wrapper.port.is_some() {
      Ok(wrapper.port)
    } else {
      Err(D::Error::missing_field("port"))
    }
  }

  fn is_false(b: &bool) -> bool {
    !*b
  }

  fn deserialize_bool<'de, D: Deserializer<'de>>(
    d: D,
  ) -> Result<bool, D::Error> {
    let num = u8::deserialize(d)?;
    Ok(num > 0)
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Response {
  pub id: NodeId,

  #[serde(
    with = "compact::values",
    default,
    skip_serializing_if = "Vec::is_empty"
  )]
  pub values: Vec<SocketAddr>,

  #[serde(
    rename = "nodes",
    with = "compact::nodes_v4",
    default,
    skip_serializing_if = "Vec::is_empty"
  )]
  pub nodes_v4: Vec<NodeHandle>,

  #[serde(
    rename = "nodes6",
    with = "compact::nodes_v6",
    default,
    skip_serializing_if = "Vec::is_empty"
  )]
  pub nodes_v6: Vec<NodeHandle>,

  // Only present in response to GetPeers
  #[serde(
    with = "serde_bytes",
    default,
    skip_serializing_if = "Option::is_none"
  )]
  pub token: Option<Vec<u8>>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Error {
  pub code: u8,
  pub message: String,
}

impl Serialize for Error {
  fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let mut seq = s.serialize_seq(Some(2))?;
    seq.serialize_element(&self.code)?;
    seq.serialize_element(&self.message)?;
    seq.end()
  }
}

impl<'de> Deserialize<'de> for Error {
  fn deserialize<D>(d: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    struct ErrorVisitor;

    impl<'de> Visitor<'de> for ErrorVisitor {
      type Value = Error;

      fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a list of two elements: an integer and a string")
      }

      fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
      where
        A: serde::de::SeqAccess<'de>,
      {
        let code: u8 = seq
          .next_element()?
          .ok_or_else(|| A::Error::invalid_length(0, &self))?;

        let message = seq
          .next_element()?
          .ok_or_else(|| A::Error::invalid_length(1, &self))?;

        if seq.next_element::<IgnoredAny>()?.is_some() {
          return Err(A::Error::invalid_length(3, &self));
        }

        Ok(Error { code, message })
      }
    }

    d.deserialize_seq(ErrorVisitor)
  }
}

pub mod error_code {
  // some of these codes are not used in this crate but we still list them here for completeness.
  #![allow(unused)]

  pub const GENERIC_ERROR: u8 = 201;
  pub const SERVER_ERROR: u8 = 202;
  pub const PROTOCOL_ERROR: u8 = 203;
  pub const METHOD_UNKNOWN: u8 = 204;
}