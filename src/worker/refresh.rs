use std::time::Duration;

use crate::{
  message::{FindNodeRequest, Message, MessageBody, Request},
  routing::{
    node::NodeStatus,
    table::{self, RoutingTable},
  },
  transaction::{ActionID, MIDGenerator},
};

use super::{socket::Socket, timer::Timer, ScheduledTaskCheck};

const REFRESH_INTERVAL_TIMEOUT: Duration = Duration::from_millis(6000);
const REFRESH_CONCURRENCY: usize = 4;

pub struct TableRefresh {
  id_generator: MIDGenerator,
  current_refresh_bucket: usize,
}

impl TableRefresh {
  pub fn new(id_generator: MIDGenerator) -> Self {
    TableRefresh {
      id_generator,
      current_refresh_bucket: 0,
    }
  }

  pub fn action_id(&self) -> ActionID {
    self.id_generator.action_id()
  }

  pub async fn continue_refresh(
    &mut self,
    table: &mut RoutingTable,
    socket: &Socket,
    timer: &mut Timer<ScheduledTaskCheck>,
  ) {
    // cycle the counted index.
    if self.current_refresh_bucket == table::MAX_BUCKETS {
      self.current_refresh_bucket = 0;
    }
    // The flip_bit function is used to calculate a target node ID to refresh a particular bucket in the routing table.
    // The bucket is identified using the current_refresh_bucket field of the TableRefresh struct.

    // In the DHT network, node IDs are typically represented as fixed-length bit strings.
    // To calculate the target ID for a given bucket, we need to flip the bit at the index corresponding to the bucket number.
    // This is because the DHT network organizes nodes into buckets based on their proximity to a particular node ID.
    // The closer a node's ID is to a particular target ID, the higher the probability that the node has information about other nodes with similar IDs.

    // By flipping the bit at the index corresponding to the current bucket number,
    // we can generate a target ID that is similar to the node ID, but with a different bit at the appropriate position.
    // This enables us to query nodes in the network that have IDs similar to the target ID,
    // which increases the chances of discovering new nodes to add to the routing table.
    let target_id = table.node_id().flip_bit(self.current_refresh_bucket);

    log::debug!(
      "Performing a refresh for bucket {}",
      self.current_refresh_bucket
    );

    let nodes = table
      .closest_nodes(target_id)
      .filter(|n| n.status() == NodeStatus::Questionable)
      .filter(|n| !n.recently_requested_from())
      .take(REFRESH_CONCURRENCY)
      .map(|node| *node.handle())
      .collect::<Vec<_>>();

    // Ping the closest questionable nodes.
    for node in nodes {
      // Generate a transaction id for the request.
      let trans_id = self.id_generator.generate();

      // Construct the message.
      let find_node_req = FindNodeRequest {
        id: table.node_id(),
        target: target_id,
        want: None,
      };
      let find_node_msg = Message {
        transaction_id: trans_id.as_ref().to_vec(),
        body: MessageBody::Request(Request::FindNode(find_node_req)),
      };
      let find_node_msg = find_node_msg.encode();

      // Send the message.
      if let Err(error) = socket.send(&find_node_msg, node.addr).await {
        log::error!("TableRefresh failed to send a refresh message: {}", error);
      }

      // Mark that we requested from the node.
      if let Some(node) = table.find_node_mut(&node) {
        node.local_request();
      }
    }

    timer
      .schedule_in(REFRESH_INTERVAL_TIMEOUT, ScheduledTaskCheck::TableRefresh);

    self.current_refresh_bucket += 1;
  }
}
