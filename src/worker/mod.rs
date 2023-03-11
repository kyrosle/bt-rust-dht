use std::io;

use crate::transaction::TransactionID;
use thiserror::Error;

mod bootstrap;
mod handler;
mod lookup;
mod refresh;
mod socket;
mod timer;

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

#[derive(Error, Debug)]
pub enum WorkerError {
  #[error("invalid bencode data")]
  InvalidBencodeDe(#[source] serde_bencoded::DeError),
  #[error("invalid bencode data")]
  InvalidBencodeSer(#[source] serde_bencoded::SerError),
  #[error("received unsolicited response")]
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
