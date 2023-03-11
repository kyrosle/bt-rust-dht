use std::{
  collections::{HashMap, HashSet},
  net::SocketAddr,
};

use crate::{
  id::NodeId,
  transaction::{MIDGenerator, TransactionID},
  IpVersion,
};

use super::{
  socket::Socket,
  timer::{Timeout, Timer},
  ScheduledTaskCheck,
};

pub struct TableBootstrap {
  ip_version: IpVersion,
  table_id: NodeId,
  routers: HashSet<String>,
  router_addresses: HashSet<SocketAddr>,
  id_generator: MIDGenerator,
  starting_nodes: HashSet<SocketAddr>,
  active_message: HashMap<TransactionID, Timeout>,
  current_bootstrap_bucket: usize,
  initial_responses: HashSet<SocketAddr>,
  initial_responses_expected: usize,
  state: State,
  bootstrap_attempt: u64,
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
    ip_version: IpVersion,
    table_id: NodeId,
    id_generator: MIDGenerator,
    routers: HashSet<String>,
    nodes: HashSet<SocketAddr>,
  ) -> Self {
    TableBootstrap {
      ip_version,
      table_id,
      routers,
      router_addresses: HashSet::new(),
      id_generator,
      starting_nodes: nodes,
      active_message: HashMap::new(),
      current_bootstrap_bucket: 0,
      initial_responses: HashSet::new(),
      initial_responses_expected: 0,
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
    if (self.state == State::Bootstrapped) == (new_state == State::Bootstrapped)
    {
      self.state = new_state;
      false
    } else {
      log::info!(
        "{}: TableBootstrap state change {:?} -> {:?} (from: {})",
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

    if self.routers.is_empty() {
      self.bootstrap_attempt = 0;
      return self.set_state(State::Bootstrapped, line!());
    }

    // resolve() !!

    true
  }
}
