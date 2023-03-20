// TODO:
// unrecognized requests which contain either
// an 'info_hash' or 'target' arguments should be
// interpreted as 'find_node' as per Mainline DHT extensions.

use serde::{Deserialize, Serialize};

use crate::{InfoHash, NodeId};

use super::{
  utils::{port, want},
  Want,
};

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
