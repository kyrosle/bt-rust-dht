use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::{compact, routing::node::NodeHandle, NodeId};

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
