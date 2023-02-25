//! Reference link: http://bittorrent.org/beps/bep_0005.html
//!
//! ## Contact Encoding
//! Contact information for `peers` is encoded as a `6-byte string`.
//! Also known as "Compact IP-address/port info" the 4-byte IP address
//! is in network byte order with the 2-byte port in network byte order concatenated onto the end.
//!
//! Contact information for `nodes` is encoded as a `26-byte string`.
//! Also known as "Compact node info" the 20-byte Node ID in network byte order
//! has the compact IP-address/port info concatenated to the end.
use crate::compact;
use std::{fmt, net::SocketAddr};

use serde::{
  de::{Error as _, IgnoredAny, Visitor},
  ser::SerializeSeq,
  Deserialize, Serialize,
};

use crate::{
  id::{InfoHash, NodeId},
  routing::node::NodeHandle,
};

/// DHT Queries: ping/find_node/get_peers/announce_peer
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Message {
  /// Every message has a key "t" with a string value representing a transaction ID.
  ///
  /// This transaction ID is generated by the `querying node` and is echoed in the response,
  /// so responses may be correlated with multiple queries to the same node.
  ///
  /// The transaction ID should be encoded as a short string of binary numbers,
  /// `typically 2 characters` are enough as they cover 2^16 outstanding queries.
  #[serde(rename = "t", with = "serde_bytes")]
  pub transaction_id: Vec<u8>,
  #[serde(flatten)]
  pub body: MessageBody,
}

/// Every message also has a key "y" with a single character value describing the type of message.
///
/// The value of the "y" key is one of "q" for query, "r" for response, or "e" for error.
///
/// A key "v" should be included in every message with a client version string.
/// (Not all implementations include a "v" key so clients should not assume its presence.)
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
#[serde(tag = "y")]
pub enum MessageBody {
  /// Queries, or KRPC message dictionaries with a "y" value of "q",
  /// contain two additional keys; "q" and "a".
  /// - Key "q" has a string value containing the method name of the query.
  /// - Key "a" has a dictionary value containing named arguments to the query.
  #[serde(rename = "q")]
  Request(Request),
  /// Responses, or KRPC message dictionaries with a "y" value of "r",
  /// contain one additional key "r".
  /// - The value of "r" is a dictionary containing named return values.
  /// - Response messages are sent upon successful completion of a query.
  #[serde(rename = "r", with = "unflatten::response")]
  Response(Response),
  /// Errors, or KRPC message dictionaries with a "y" value of "e",
  /// contain one additional key "e".
  /// - The value of "e" is a list.
  /// - The first element is an integer representing the error code.
  /// - The second element is a string containing the error message.
  ///
  /// Errors are sent when a query cannot be fulfilled.
  ///
  /// The following table describes the possible error codes [`error_code`]
  #[serde(rename = "e", with = "unflatten::error")]
  Error(Error),
}

/// Helper to serialize/deserialize `Response`/`Error` in `MessageBody`
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

/// All queries have an "id" key and value containing the node ID of the querying node.
///
/// All responses have an "id" key and value containing the node ID of the responding node.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
#[serde(tag = "q", content = "a")]
#[serde(rename_all = "snake_case")]
pub enum Request {
  Ping(PingRequest),
  FindNode(FindNodeRequest),
  GetPeers(GetPeersRequest),
  AnnouncePeer(AnnouncePeerRequest),
}

/// The most basic query is a ping.
///
/// "q" = "ping" A ping query has a single argument("id").
///
/// The appropriate response to a ping has a single key "id" containing the node ID of the responding node.
///
/// ## Example Packets:
/// ```json
/// ping_Query = {
///   "t": "aa",
///   "y": "q",
///   "q": "ping",
///   "a": {
///     "id": "abcdefghij0123456789"
///   }
/// }
/// Response = {
///   "t": "aa",
///   "y": "r",
///   "r": {
///     "id": "mnopqrstuvwxyz123456"
///   }
/// }
/// ```
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct PingRequest {
  /// "id" the value is a `20-byte string` containing the `senders node ID` in network byte order.
  pub id: NodeId,
}

/// Find node is used to find the contact information for a node given its ID.
///
/// "q" == "find_node" A find_node query has two arguments("id", "target").
///
/// When a node receives a find_node query,
/// it should respond with a key "nodes"
/// and value of a string containing the compact node info
/// for the target node or the K (8) closest good nodes in its own routing table.
///
/// ## Example Packets:
/// ```json
/// find_node Query = {
///   "t": "aa",
///   "y": "q",
///   "q": "find_node",
///   "a": {
///     "id": "abcdefghij0123456789",
///     "target": "mnopqrstuvwxyz123456"
///   }
/// }
/// Response = {
///   "t": "aa",
///   "y": "r",
///   "r": {
///     "id": "0123456789abcdefghij",
///     "nodes": "def456..."
///   }
/// }
/// ```
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct FindNodeRequest {
  /// "id" containing the node ID of the querying node,
  pub id: NodeId,
  /// "target" containing the ID of the node sought by the queryer.
  pub target: NodeId,

  #[serde(with = "want", default, skip_serializing_if = "Option::is_none")]
  pub want: Option<Want>,
}

/// Get peers associated with a torrent infohash.
///
/// "q" = "get_peers" A get_peers query has two arguments("id", "info_hash").
///
/// - If the queried node has peers for the infohash,
///    they are returned in a key "values" as a list of strings.
///    Each string containing "compact" format peer information for a single peer.
/// - If the queried node has no peers for the infohash,
///    a key "nodes" is returned containing the K nodes in the queried nodes routing table
///    closest to the infohash supplied in the query.
///
/// In either case a "token" key is also included in the return value.
///
/// The token value is a required argument for a future announce_peer query.
///
/// The token value should be a short binary string.
///
/// ## Example Packets:
/// ```json
/// get_peers Query = {
///   "t": "aa",
///   "y": "q",
///   "q": "get_peers",
///   "a": {
///     "id": "abcdefghij0123456789",
///     "info_hash": "mnopqrstuvwxyz123456"
///   }
/// }
/// Response with peers = {
///   "t": "aa",
///   "y": "r",
///   "r": {
///     "id": "abcdefghij0123456789",
///     "token": "aoeusnth",
///     "values": [
///       "axje.u",
///       "idhtnm"
///     ]
///   }
/// }
/// Response with closest nodes = {
///   "t": "aa",
///   "y": "r",
///   "r": {
///     "id": "abcdefghij0123456789",
///     "token": "aoeusnth",
///     "nodes": "def456..."
///   }
/// }
/// ```
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct GetPeersRequest {
  /// "id" containing the node ID of the querying node.
  pub id: NodeId,
  /// "info_hash" containing the infohash of the torrent.
  pub info_hash: InfoHash,

  #[serde(with = "want", default, skip_serializing_if = "Option::is_none")]
  pub want: Option<Want>,
}

/// Announce that the peer,
/// controlling the querying node, is downloading a torrent on a port.
///
/// announce_peer has four arguments("id", "info_hash", "port", "token").
///
/// The queried node must verify that the token
/// was previously sent to the same IP address as the querying node.
///
/// Then the queried node should store the IP address of the querying node
/// and the supplied port number under the infohash in its store of peer contact information.
///
/// ## Example Packets:
/// ```json
/// announce_peers Query = {
///   "t": "aa",
///   "y": "q",
///   "q": "announce_peer",
///   "a": {
///     "id": "abcdefghij0123456789",
///     "implied_port": 1,
///     "info_hash": "mnopqrstuvwxyz123456",
///     "port": 6881,
///     "token": "aoeusnth"
///   }
/// }
/// Response = {
///   "t": "aa",
///   "y": "r",
///   "r": {
///     "id": "mnopqrstuvwxyz123456"
///   }
/// }
/// ```
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct AnnouncePeerRequest {
  /// "id" containing the node ID of the querying node.
  pub id: NodeId,
  /// "info_hash" containing the infohash of the torrent.
  pub info_hash: InfoHash,
  /// "port" containing the port as an integer.
  #[serde(with = "port", flatten)]
  pub port: Option<u16>,
  #[serde(with = "serde_bytes")]
  /// "token" received in response to a previous get_peers query.
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

/// The following table describes the possible error codes:
pub mod error_code {
  // some of these codes are not used in this crate but we still list them here for completeness.
  #![allow(unused)]

  pub const GENERIC_ERROR: u8 = 201;
  pub const SERVER_ERROR: u8 = 202;
  pub const PROTOCOL_ERROR: u8 = 203;
  pub const METHOD_UNKNOWN: u8 = 204;
}
