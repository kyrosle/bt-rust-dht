use std::{
  collections::{HashMap, HashSet},
  net::SocketAddr,
  time::Duration,
};

use crate::{
  id::NodeId,
  message::{FindNodeRequest, Message, MessageBody, Request},
  routing::{
    bucket::Bucket,
    node::{NodeHandle, NodeStatus},
    table::{self, RoutingTable},
  },
  transaction::{ActionID, MIDGenerator, TransactionID},
  IpVersion,
};

use super::{
  resolve,
  socket::Socket,
  timer::{Timeout, Timer},
  BootstrapTimeout, ScheduledTaskCheck,
};

const INITIAL_TIMEOUT: Duration = Duration::from_millis(2500);
const NODE_TIMEOUT: Duration = Duration::from_millis(500);
const NO_NETWORK_TIMEOUT: Duration = Duration::from_secs(5);
const PERIODIC_CHECK_TIMEOUT: Duration = Duration::from_secs(5);
const GOOD_NODE_THRESHOLD: usize = 10;

const PINGS_PER_BUCKET: usize = 8;

/// Bootstrap for startup the table-routing
pub struct TableBootstrap {
  /// A string representing the name of the node that is performing the bootstrap.
  name: String,
  /// An enum representing the IP version of the node, which could be IPv4 or IPv6.
  ip_version: IpVersion,
  /// Representing the unique ID of the node's routing table.
  table_id: NodeId,
  /// Representing the IP addresses of the known routers in the network
  routers: HashSet<String>,
  /// Representing the network addresses of the known routers in the network
  router_addresses: HashSet<SocketAddr>,
  /// An ID generator that generates transaction IDs used for sending messages in the network.
  id_generator: MIDGenerator,
  /// Representing the initial nodes to contact for bootstrap.
  starting_nodes: HashSet<SocketAddr>,
  /// Used for tracking active messages in the network.
  active_message: HashMap<TransactionID, Timeout>,
  /// An integer representing the current bucket being used for bootstrap.
  current_bootstrap_bucket: usize,
  /// Representing the initial responses received during bootstrap
  initial_responses: HashSet<SocketAddr>,
  /// An integer representing the expected number of initial responses to receive during bootstrap.
  initial_responses_expected: usize,
  /// An enum representing the state of the bootstrap process.
  state: State,
  /// An integer representing the number of times bootstrap has been attempted.
  bootstrap_attempt: u64,
  /// Representing the last send error encountered during bootstrap.
  last_send_error: Option<std::io::ErrorKind>,
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
enum State {
  Bootstrapping,
  Bootstrapped,
  // The starting state or state after a bootstrap has failed and
  // new has been schedule after a timeout.
  IdleBeforeReBootstrap,
}

impl TableBootstrap {
  pub fn new(
    name: String,
    ip_version: IpVersion,
    table_id: NodeId,
    id_generator: MIDGenerator,
    routers: HashSet<String>,
    nodes: HashSet<SocketAddr>,
  ) -> Self {
    TableBootstrap {
      name,
      ip_version,
      table_id,
      routers,
      router_addresses: HashSet::new(),
      id_generator,
      starting_nodes: nodes,
      active_message: HashMap::new(),
      current_bootstrap_bucket: 0,
      initial_responses: HashSet::new(),
      initial_responses_expected: 8,
      state: State::IdleBeforeReBootstrap,
      bootstrap_attempt: 0,
      last_send_error: None,
    }
  }

  pub fn router_addresses(&self) -> &HashSet<SocketAddr> {
    &self.router_addresses
  }

  pub fn is_bootstrapped(&self) -> bool {
    self.state == State::Bootstrapped
  }

  /// Return true if we switched between Bootstrapped and not being Bootstrapped.
  fn set_state(&mut self, new_state: State, from: u32) -> bool {
    log::debug!(
      "[{}] Bootstrap State want to change: {:?} -> {:?}",
      self.name,
      self.state,
      new_state
    );
    if (self.state == State::Bootstrapped) == (new_state == State::Bootstrapped)
    {
      self.state = new_state;
      false
    } else {
      log::info!(
        "[{}] {}: TableBootstrap state change {:?} -> {:?} (from: {})",
        self.name,
        self.ip_version,
        self.state,
        new_state,
        from
      );
      self.state = new_state;

      true
    }
  }

  /// Return true if the bootstrap state changed.
  pub async fn start(
    &mut self,
    socket: &Socket,
    timer: &mut Timer<ScheduledTaskCheck>,
  ) -> bool {
    self.bootstrap_attempt += 1;

    // If we have no bootstrap contacts it means we are the first node in the network and
    // other would bootstrap against us. We consider this node as already bootstrapped.
    if self.routers.is_empty() && self.starting_nodes.is_empty() {
      log::debug!(
        "[{}] Router is empty, I may be the first node in this network.",
        self.name
      );
      self.bootstrap_attempt = 0;
      return self.set_state(State::Bootstrapped, line!());
    }

    // resolve the router, convert String into SocketAddress, with tokio::net::lookup_host.
    self.router_addresses = resolve(&self.routers, socket.ip_version()).await;
    if !self.routers.is_empty() && self.router_addresses.is_empty() {
      // log::debug!(
      //   "[{}] resolve the router_address counts: {}",
      //   self.name,
      //   self.router_addresses.len()
      // );

      // This doesn't need to be counted as a failed bootstrap attempt because we have
      // not yet pinged any of the routers (bootstrap nodes) and thus don't need to
      // do the exponential back off as to not stress them.
      self.bootstrap_attempt = 0;
      idle_timeout_in(timer, NO_NETWORK_TIMEOUT);
      self.set_state(State::IdleBeforeReBootstrap, line!());
    }

    // Reset the bootstrap state
    self.active_message.clear();
    self.current_bootstrap_bucket = 0;

    // In the initial round, we send the requests to contracts (nodes and routers)
    // who are not in our routing table. Because of that, we don't care who we
    // receive a response from, only that we receive sufficient number of unique
    // ones. Thus we use the same transaction id for all of them.

    // After the initial round we are sending only to nodes from the routing table
    // table, so we use unique transaction id per node.
    let trans_id = self.id_generator.generate();

    // Set a timer to begin the actual bootstrap.
    let timeout = transaction_timeout_in(timer, INITIAL_TIMEOUT, trans_id);

    self.active_message.insert(trans_id, timeout);

    let find_node_msg = Message {
      transaction_id: trans_id.as_ref().to_vec(),
      body: MessageBody::Request(Request::FindNode(FindNodeRequest {
        id: self.table_id,
        target: self.table_id,
        want: None, // we want only contacts of the same address family we have.
      })),
    }
    .encode();

    // Ping all initial routers and nodes.
    self.initial_responses_expected = 0;
    self.initial_responses.clear();

    for addr in self
      .router_addresses
      .iter()
      .chain(self.starting_nodes.iter())
    {
      log::trace!(
        "[{}] sending initial request to {}",
        &self.name,
        addr.to_string()
      );
      match socket.send(&find_node_msg, *addr).await {
        Ok(()) => {
          if self.initial_responses_expected < PINGS_PER_BUCKET {
            self.initial_responses_expected += 1;
          }
        }
        Err(e) => {
          log::error! {
            "[{}] {}: Failed to send bootstrap message to router: {}",
            self.name,
            self.ip_version,
            e
          };
          self.last_send_error = Some(e.kind());
        }
      }
    }

    if self.initial_responses_expected > 0 {
      self.last_send_error = None;
      self.set_state(State::Bootstrapping, line!())
    } else {
      // Nothing was sent, wait for timeout to restart the bootstrap.

      // This doesn't need to be counted as a failed bootstrap attempt because
      // we have not yet pinged any of the routers (bootstrap nodes) and thus don't
      // need to do the exponential back off so as to not stress them.
      self.bootstrap_attempt = 0;
      idle_timeout_in(timer, NO_NETWORK_TIMEOUT);
      self.set_state(State::IdleBeforeReBootstrap, line!())
    }
  }

  fn calculate_retry_duration(&self) -> Duration {
    // `bootstrap_attempt` is always assumed to be >= one, but check for it anyway.
    let n = self.bootstrap_attempt.max(1);
    const BASE: u64 = 2;
    // Max is somewhere around 8.5 mins.
    Duration::from_secs(BASE.pow(n.min(9) as u32))
  }

  pub fn action_id(&self) -> ActionID {
    self.id_generator.action_id()
  }

  pub async fn recv_response(
    &mut self,
    addr: SocketAddr,
    trans_id: &TransactionID,
    table: &mut RoutingTable,
    socket: &Socket,
    timer: &mut Timer<ScheduledTaskCheck>,
  ) -> bool {
    // Process the message transaction id.
    let timeout = if let Some(t) = self.active_message.get(trans_id) {
      *t
    } else {
      log::debug!(
        "[{}] {}: Received expired/unsolicited node response for an active table bootstrap",
        self.name,
        self.ip_version
      );
      // Return that the state has not changed.
      return false;
    };

    log::trace!(
      "[{}] received response, active message: {}",
      self.name,
      self.active_message.len()
    );

    // In the initial round all the messages have the same transaction id so clear it
    // only after we receive sufficient number of unique response. After the initial round,
    // every message has its own transaction id so clear it immediately.
    if self.current_bootstrap_bucket == 0 {
      self.initial_responses.insert(addr);

      if self.initial_responses.len() >= self.initial_responses_expected {
        timer.cancel(timeout);
        self.active_message.remove(trans_id);
      }
    } else {
      timer.cancel(timeout);
      self.active_message.remove(trans_id);
    }

    // Check if we need to bootstrap on the next bucket.
    if self.active_message.is_empty() {
      self.bootstrap_next_bucket(table, socket, timer).await
    } else {
      false
    }
  }

  pub async fn recv_timeout(
    &mut self,
    timeout: &BootstrapTimeout,
    table: &mut RoutingTable,
    socket: &Socket,
    timer: &mut Timer<ScheduledTaskCheck>,
  ) -> bool {
    match timeout {
      BootstrapTimeout::Transaction(trans_id) => {
        self
          .handle_transaction_timeout(table, socket, timer, trans_id)
          .await
      }
      BootstrapTimeout::IdleWakeUp => {
        self.handle_wake_up_timeout(table, socket, timer).await
      }
    }
  }

  async fn handle_transaction_timeout(
    &mut self,
    table: &mut RoutingTable,
    socket: &Socket,
    timer: &mut Timer<ScheduledTaskCheck>,
    trans_id: &TransactionID,
  ) -> bool {
    if self.active_message.remove(trans_id).is_none() {
      log::warn!(
        "[{}] {}: Received expired/unsolicited node timeout in table bootstrap",
        self.name,
        self.ip_version
      );
      return false;
    }

    match self.state {
      State::Bootstrapping => {
        // Check if we need to bootstrap on the next bucket.
        if self.active_message.is_empty() {
          self.bootstrap_next_bucket(table, socket, timer).await
        } else {
          false
        }
      }
      State::Bootstrapped => false,
      State::IdleBeforeReBootstrap => false,
    }
  }

  async fn handle_wake_up_timeout(
    &mut self,
    table: &RoutingTable,
    socket: &Socket,
    timer: &mut Timer<ScheduledTaskCheck>,
  ) -> bool {
    match self.state {
      State::Bootstrapping => false,
      State::Bootstrapped => {
        if table.num_good_nodes() < GOOD_NODE_THRESHOLD {
          self.start(socket, timer).await
        } else {
          idle_timeout_in(timer, PERIODIC_CHECK_TIMEOUT);
          false
        }
      }
      State::IdleBeforeReBootstrap => self.start(socket, timer).await,
    }
  }

  async fn bootstrap_next_bucket(
    &mut self,
    table: &mut RoutingTable,
    socket: &Socket,
    timer: &mut Timer<ScheduledTaskCheck>,
  ) -> bool {
    log::debug!(
      "[{}] {}: TableBootstrap::bootstrap_next_bucket {} {}",
      self.name,
      self.ip_version,
      self.current_bootstrap_bucket,
      table::MAX_BUCKETS
    );

    // log::debug!("[{}] routing table: {:?}", self.name, table);
    loop {
      if self.current_bootstrap_bucket >= table::MAX_BUCKETS {
        if table.num_good_nodes() >= GOOD_NODE_THRESHOLD {
          self.bootstrap_attempt = 0;
          idle_timeout_in(timer, PERIODIC_CHECK_TIMEOUT);
          return self.set_state(State::Bootstrapped, line!());
        } else {
          let time = self.calculate_retry_duration();
          log::trace!(
            "[{}] good is less than threshold. Idle timeout {}",
            self.name,
            time.as_secs()
          );
          idle_timeout_in(timer, time);
          return self.set_state(State::IdleBeforeReBootstrap, line!());
        }
      }

      let target_id = table.node_id().flip_bit(self.current_bootstrap_bucket);

      // Get the optimal iterator to bootstrap the current bucket
      let nodes: Vec<_> = if self.current_bootstrap_bucket == 0
        || self.current_bootstrap_bucket == 1
      {
        table
          .closest_nodes(target_id)
          .filter(|n| n.status() == NodeStatus::Questionable)
          .take(PINGS_PER_BUCKET)
          .map(|node| *node.handle())
          .collect()
      } else {
        let mut buckets =
          table.buckets().skip(self.current_bootstrap_bucket - 2);

        let dummy_bucket = Bucket::new();

        let percent_25_bucket = if let Some(bucket) = buckets.next() {
          bucket.iter()
        } else {
          dummy_bucket.iter()
        };

        let percent_50_bucket = if let Some(bucket) = buckets.next() {
          bucket.iter()
        } else {
          dummy_bucket.iter()
        };

        let percent_100_bucket = if let Some(bucket) = buckets.next() {
          bucket.iter()
        } else {
          dummy_bucket.iter()
        };

        // TODO: Figure out why chaining then in reverse gives us more total nodes on average,
        // perhaps it allows us to fill up the lower buckets faster at the cost of less nodes
        // in the higher buckets (since lower buckets are very easy to fill)...
        // Although it should even out since we are stagnating buckets, so doing it in reverse
        // may make sense since on the 3rd iteration, it allows us to ping questionable nodes
        // in our first bucket right off the bat.
        percent_25_bucket
          .chain(percent_50_bucket)
          .chain(percent_100_bucket)
          .filter(|n| n.status() == NodeStatus::Questionable)
          .take(PINGS_PER_BUCKET)
          .map(|node| *node.handle())
          .collect()
      };

      // log::debug!("[{}] routing table: {:?}", self.name, table);
      // log::debug!("[{}] closest nodes count: {}", self.name, nodes.len());

      self.current_bootstrap_bucket += 1;
      // log::debug!("[{}] current_bootstrap --> {}",self.name, self.current_bootstrap_bucket);

      // If we failed to send any message, try again on the next bucket.
      if self
        .send_bootstrap_requests(&nodes, target_id, table, socket, timer)
        .await
      {
        return self.set_state(State::Bootstrapping, line!());
      }
    }
  }

  /// If this returns `false` it means the request wasn't sent to any node (either
  /// because there were no nodes or because all the sends failed). We should proceed
  /// to the next bucket in that case.
  async fn send_bootstrap_requests(
    &mut self,
    nodes: &[NodeHandle],
    target_id: NodeId,
    table: &mut RoutingTable,
    socket: &Socket,
    timer: &mut Timer<ScheduledTaskCheck>,
  ) -> bool {
    let mut messages_sent = 0;
    // log::debug!("[{}] send bootstrap requests, nodes count: {}", self.name, nodes.len());

    for node in nodes {
      // Generate a transaction id
      let trans_id = self.id_generator.generate();

      let find_node_msg = Message {
        transaction_id: trans_id.as_ref().to_vec(),
        body: MessageBody::Request(Request::FindNode(FindNodeRequest {
          id: table.node_id(),
          target: target_id,
          want: None,
        })),
      }
      .encode();

      // Add a timeout for the node
      let timeout = transaction_timeout_in(timer, NODE_TIMEOUT, trans_id);

      // Send the message to the node
      if let Err(error) = socket.send(&find_node_msg, node.addr).await {
        log::error!(
          "[{}] {}: Could not send a bootstrap message: {}",
          self.name,
          self.ip_version,
          error
        );
        continue;
      }

      // Mark that we requested from the node
      if let Some(node) = table.find_node_mut(node) {
        node.local_request();
      }

      // Create an entry for the timeout in the map
      self.active_message.insert(trans_id, timeout);

      messages_sent += 1;
    }

    messages_sent > 0
  }
}

fn transaction_timeout_in(
  timer: &mut Timer<ScheduledTaskCheck>,
  duration: Duration,
  trans_id: TransactionID,
) -> Timeout {
  timer.schedule_in(
    duration,
    ScheduledTaskCheck::BootstrapTimeout(BootstrapTimeout::Transaction(
      trans_id,
    )),
  )
}

fn idle_timeout_in(
  timer: &mut Timer<ScheduledTaskCheck>,
  duration: Duration,
) -> Timeout {
  timer.schedule_in(
    duration,
    ScheduledTaskCheck::BootstrapTimeout(BootstrapTimeout::IdleWakeUp),
  )
}
