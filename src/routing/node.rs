use std::fmt;
use std::net::SocketAddr;

use crate::id::NodeId;

/// Node id + its socket address.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct NodeHandle {
  pub id: NodeId,
  pub addr: SocketAddr,
}

impl NodeHandle {
  pub fn new(id: NodeId, addr: SocketAddr) -> Self {
    Self { id, addr }
  }
}

impl fmt::Debug for NodeHandle {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}@{:?}", self.id, self.addr)
  }
}
