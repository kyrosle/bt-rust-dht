//! Dht is composed of nodes and stores the location of peers.
//!
//! BitTorrent client including a DHT node, which is used to contract
//! other nodes in the DHT to get the location of peers to download from
//! using the BitTorrent protocol.

pub mod compact;
pub mod id;
pub mod message;
pub mod routing;
pub mod storage;
pub mod token;
pub mod transaction;

pub mod builder;
pub mod worker;

pub mod test;

pub type IpVersion = crate::worker::IpVersion;

use async_trait::async_trait;
use std::{io, net::SocketAddr};

#[async_trait]
pub trait SocketTrait {
  async fn send_to(&self, buf: &[u8], target: &SocketAddr) -> io::Result<()>;
  async fn recv_from(
    &mut self,
    buf: &mut [u8],
  ) -> io::Result<(usize, SocketAddr)>;
  fn local_addr(&self) -> io::Result<SocketAddr>;
}
