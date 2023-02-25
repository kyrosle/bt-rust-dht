use std::fmt;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crate::id::NodeId;

/// Maximum wait period before a node becomes questionable.
const MAX_LAST_SEEN_MINS: u64 = 15;

/// Maximum number of requests before a Questionable node becomes Bad.
const MAX_REFRESH_REQUESTS: usize = 2;

/// Status of the node.
///
/// Ordering of the enumeration is essential,
/// variants higher up are considered to be less
/// than those further down.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeStatus {
  Bad,
  Questionable,
  Good,
}

/// Node participating in the dht.
#[derive(Clone, Eq)]
pub struct Node {
  /// The information handler.Such as Node id, and its socket address.
  handle: NodeHandle,

  /// A timestamp recording the last request happened.
  last_request: Option<Instant>,
  /// A timestamp recording the last response happened.
  last_response: Option<Instant>,
  /// A timestamp recording the last local request happened.
  last_local_request: Option<Instant>,
  /// Record the requests which should refreshing.
  refresh_requests: usize,
}

impl Node {
  /// Create a new node that has recently responded to us,
  /// but never requested from us.
  pub fn as_good(id: NodeId, addr: SocketAddr) -> Node {
    Node {
      handle: NodeHandle { id, addr },
      last_response: Some(Instant::now()),
      last_request: None,
      last_local_request: None,
      refresh_requests: 0,
    }
  }

  /// Create a questionable node that has recently responded to us,
  /// but never requested from us.
  pub fn as_questionable(id: NodeId, addr: SocketAddr) -> Node {
    let last_response_offset = Duration::from_secs(MAX_LAST_SEEN_MINS * 60);
    let last_response =
      Instant::now().checked_sub(last_response_offset).unwrap();

    Node {
      handle: NodeHandle { id, addr },
      last_response: Some(last_response),
      last_request: None,
      last_local_request: None,
      refresh_requests: 0,
    }
  }

  /// Create a bad node that has recently responded to us,
  /// but never requested from us.
  pub fn as_bad(id: NodeId, addr: SocketAddr) -> Node {
    Node {
      handle: NodeHandle { id, addr },
      last_response: None,
      last_request: None,
      last_local_request: None,
      refresh_requests: 0,
    }
  }

  pub fn update(&mut self, other: Node) {
    assert_eq!(self.handle, other.handle);

    let self_status = self.status();
    let other_status = other.status();

    match (self_status, other_status) {
      (NodeStatus::Good, NodeStatus::Good) => {
        *self = Node {
          handle: self.handle,
          last_response: other.last_response,
          last_request: self.last_request,
          last_local_request: self.last_local_request,
          refresh_requests: 0,
        };
      }
      (NodeStatus::Good, NodeStatus::Questionable) => {}
      (NodeStatus::Good, NodeStatus::Bad) => {}
      (NodeStatus::Questionable, NodeStatus::Good) => {
        *self = other;
      }
      (NodeStatus::Questionable, NodeStatus::Questionable) => {}
      (NodeStatus::Questionable, NodeStatus::Bad) => {}
      (NodeStatus::Bad, NodeStatus::Good) => {
        *self = other;
      }
      (NodeStatus::Bad, NodeStatus::Questionable) => {
        *self = other;
      }
      (NodeStatus::Bad, NodeStatus::Bad) => {}
    }
  }

  /// Record that we sent the node a request.
  pub fn local_request(&mut self) {
    self.last_local_request = Some(Instant::now());

    if self.status() != NodeStatus::Good {
      self.refresh_requests = self.refresh_requests.saturating_add(1);
    }
  }

  /// Record that the node sent us a request.
  pub fn remote_request(&mut self) {
    self.last_request = Some(Instant::now());
  }

  /// Return true if we have sent this node a request recently.
  pub fn recently_requested_from(&self) -> bool {
    if let Some(time) = self.last_local_request {
      // 30 second may reasonable?
      time > Instant::now().checked_sub(Duration::from_secs(30)).unwrap()
    } else {
      false
    }
  }

  pub fn id(&self) -> NodeId {
    self.handle.id
  }

  pub fn addr(&self) -> SocketAddr {
    self.handle.addr
  }

  pub fn handle(&self) -> &NodeHandle {
    &self.handle
  }

  /// Current status of the node.
  ///
  /// The specification says:
  ///
  /// https://www.bittorrent.org/beps/bep_0005.html
  ///
  /// Specify good node:
  /// - has responded to one of our queries within the last `15 minutes`.
  /// - has ever responded to one of our queries
  ///   and has sent us a query within the last `15 minutes`.
  ///
  /// After 15 minutes of inactivity, a node becomes questionable.
  ///
  /// Nodes becomes bad when they fail to respond to multiple queries in a row.
  ///
  pub fn status(&self) -> NodeStatus {
    let curr_time = Instant::now();

    // Check if node has ever responded to us.
    let since_response = match self.last_response {
      Some(response_time) => curr_time - response_time,
      None => return NodeStatus::Bad,
    };

    // Check if node has recently responded to us.
    if since_response < Duration::from_secs(MAX_LAST_SEEN_MINS * 60) {
      return NodeStatus::Good;
    }

    // Check if we have request from node multiple times already without response.
    if self.refresh_requests >= MAX_REFRESH_REQUESTS {
      return NodeStatus::Bad;
    }

    // Check if the node has recently requested to us.
    if let Some(request_time) = self.last_request {
      let since_request = curr_time - request_time;

      if since_request < Duration::from_secs(MAX_LAST_SEEN_MINS * 60) {
        return NodeStatus::Good;
      }
    }

    NodeStatus::Questionable
  }

  /// Is node good or questionable?
  pub fn is_ping_able(&self) -> bool {
    // Function is moderately expensive.
    let status = self.status();
    status == NodeStatus::Good || status == NodeStatus::Questionable
  }
}

impl PartialEq<Node> for Node {
  fn eq(&self, other: &Node) -> bool {
    self.handle == other.handle
  }
}

impl core::hash::Hash for Node {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.handle.hash(state);
  }
}

impl std::fmt::Debug for Node {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Node")
      .field("id", &self.handle.id)
      .field("addr", &self.handle.addr)
      .field("last_request", &self.last_request)
      .field("last_response", &self.last_response)
      .field("refresh_requests", &self.refresh_requests)
      .finish()
  }
}

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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::routing::node::{Node, NodeStatus};
    use crate::test;

    #[test]
    fn positive_as_bad() {
        let node = Node::as_bad(test::dummy_node_id(), test::dummy_socket_addr_v4());

        assert_eq!(node.status(), NodeStatus::Bad);
    }

    #[test]
    fn positive_as_questionable() {
        let node = Node::as_questionable(test::dummy_node_id(), test::dummy_socket_addr_v4());

        assert_eq!(node.status(), NodeStatus::Questionable);
    }

    #[test]
    fn positive_as_good() {
        let node = Node::as_good(test::dummy_node_id(), test::dummy_socket_addr_v4());

        assert_eq!(node.status(), NodeStatus::Good);
    }

    #[test]
    fn positive_request_renewal() {
        let mut node = Node::as_questionable(test::dummy_node_id(), test::dummy_socket_addr_v4());

        node.remote_request();

        assert_eq!(node.status(), NodeStatus::Good);
    }

    #[test]
    fn positive_node_idle() {
        let mut node = Node::as_good(test::dummy_node_id(), test::dummy_socket_addr_v4());

        let time_offset = Duration::from_secs(super::MAX_LAST_SEEN_MINS * 60);
        let idle_time = Instant::now() - time_offset;

        node.last_response = Some(idle_time);

        assert_eq!(node.status(), NodeStatus::Questionable);
    }

    #[test]
    fn positive_node_idle_reqeusts() {
        let mut node = Node::as_questionable(test::dummy_node_id(), test::dummy_socket_addr_v4());

        for _ in 0..super::MAX_REFRESH_REQUESTS {
            node.local_request();
        }

        assert_eq!(node.status(), NodeStatus::Bad);
    }

    #[test]
    fn positive_good_status_ordering() {
        assert!(NodeStatus::Good > NodeStatus::Questionable);
        assert!(NodeStatus::Good > NodeStatus::Bad);
    }

    #[test]
    fn positive_questionable_status_ordering() {
        assert!(NodeStatus::Questionable > NodeStatus::Bad);
        assert!(NodeStatus::Questionable < NodeStatus::Good);
    }

    #[test]
    fn positive_bad_status_ordering() {
        assert!(NodeStatus::Bad < NodeStatus::Good);
        assert!(NodeStatus::Bad < NodeStatus::Questionable);
    }
}
