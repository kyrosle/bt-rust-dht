use std::{collections::HashSet, io, net::SocketAddr, time::Duration};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::{id::InfoHash, transaction::TransactionID};

mod bootstrap;
mod handler;
mod lookup;
mod refresh;
mod socket;
mod timer;

// expose the `DhtHandler` and `Socket`
pub use self::{handler::DhtHandler, socket::Socket};

#[derive(Copy, Clone, Debug)]
pub struct State {
  pub is_running: bool,
  pub bootstrapped: bool,
  pub good_node_count: usize,
  pub questionable_node_count: usize,
  pub bucket_count: usize,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum IpVersion {
  V4,
  V6,
}

impl std::fmt::Display for IpVersion {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::V4 => write!(f, "IPv4"),
      Self::V6 => write!(f, "IPv6"),
    }
  }
}

/// Task that our DHT will execute immediately.
pub enum OneShotTask {
  /// Load a new bootstrap operation into worker storage.
  StartBootstrap(),
  /// Check bootstrap status. The given sender will be notified
  /// when the bootstrap completed.
  /// with an optional timeout.
  CheckBootstrap(oneshot::Sender<bool>, Option<Duration>),
  /// Start a lookup for the given InfoHash.
  StartLookup(StartLookup),
  /// Get the local address the socket is bound to.
  GetLocalAddr(oneshot::Sender<SocketAddr>),
  /// Retrieve debug information
  GetState(oneshot::Sender<State>),
  /// Check all the node contains.
  GetNodes(oneshot::Sender<Vec<SocketAddr>>),
}

impl std::fmt::Display for OneShotTask {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      OneShotTask::StartBootstrap() => write!(f, "StartBootstrap"),
      OneShotTask::CheckBootstrap(_, _) => write!(f, "CheckBootstrap"),
      OneShotTask::StartLookup(_) => write!(f, "StartLookup"),
      OneShotTask::GetLocalAddr(_) => write!(f, "GetLocalAddr"),
      OneShotTask::GetState(_) => write!(f, "GetState"),
      OneShotTask::GetNodes(_) => write!(f, "GetNodes"),
    }
  }
}

pub struct StartLookup {
  pub info_hash: InfoHash,
  pub announce: bool,
  pub tx: mpsc::UnboundedSender<SocketAddr>,
}

/// Signifies what has timed out in the TableBootstrap class.
#[derive(Copy, Clone, Debug)]
pub enum BootstrapTimeout {
  Transaction(TransactionID),
  IdleWakeUp,
}

/// Task that our DHT will execute some time later.
#[derive(Copy, Clone, Debug)]
pub enum ScheduledTaskCheck {
  /// Check the progress of the bucket refresh.
  TableRefresh,
  /// Check the progress of the current bootstrap.
  BootstrapTimeout(BootstrapTimeout),
  /// Timeout for user waiting to get bootstrap.
  UserBootstrappedTimeout(u64),
  /// Check the progress of a current lookup.
  LookupTimeout(TransactionID),
  /// Check the progress of the lookup endgame.
  LookupEndGame(TransactionID),
}

impl std::fmt::Display for ScheduledTaskCheck {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
        ScheduledTaskCheck::TableRefresh => write!(f, "TableRefresh"),
        ScheduledTaskCheck::BootstrapTimeout(_) => write!(f, "BootstrapTimeout"),
        ScheduledTaskCheck::UserBootstrappedTimeout(_) => write!(f, "UserBootstrappedTimeout"),
        ScheduledTaskCheck::LookupTimeout(_) => write!(f, "LookupTimeout"),
        ScheduledTaskCheck::LookupEndGame(_) => write!(f, "LookupEndgame"),
    }
  }
}

#[derive(Error, Debug)]
pub enum WorkerError {
  #[error("invalid bencode data")]
  InvalidBencodeDe(#[source] serde_bencoded::DeError),
  #[error("invalid bencode data")]
  InvalidBencodeSer(#[source] serde_bencoded::SerError),
  #[error("received unsolicited response")]
  UnsolicitedResponse,
  #[error("invalid transaction id")]
  InvalidTransactionId,
  #[error("socket error")]
  SocketError(#[from] io::Error),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ActionStatus {
  /// Action is in progress
  Ongoing,
  /// Action Completed
  Completed,
}

pub async fn resolve(
  routers: &HashSet<String>,
  ip_v: IpVersion,
) -> HashSet<SocketAddr> {
  futures_util::future::join_all(routers.iter().map(tokio::net::lookup_host))
    .await
    .into_iter()
    .filter_map(|result| result.ok())
    .flatten()
    .filter(|addr| match ip_v {
      IpVersion::V4 => addr.is_ipv4(),
      IpVersion::V6 => addr.is_ipv6(),
    })
    .collect()
}
