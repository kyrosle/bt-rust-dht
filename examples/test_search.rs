use bt_rust_dht::{InfoHash, MainlineDht};
use futures_util::StreamExt;
use std::{
  net::{Ipv4Addr, SocketAddr},
  time::Duration,
};
use tokio::net::UdpSocket;

use pretty_assertions::assert_eq;

#[tokio::main]
async fn main() {
  pretty_env_logger::init();
  announce_and_lookup(AddrFamily::V4).await;
}

async fn announce_and_lookup(addr_family: AddrFamily) {
  // Start the router node for the other nodes to bootstrap against.
  let bootstrap_node_socket =
    UdpSocket::bind(localhost(addr_family)).await.unwrap();
  log::info!(
    "router node address: {}",
    bootstrap_node_socket.local_addr().unwrap()
  );

  let bootstrap_node_addr = bootstrap_node_socket.local_addr().unwrap();
  let bootstrap_node = MainlineDht::builder()
    .set_read_only(false)
    .start("router", bootstrap_node_socket)
    .unwrap();

  assert!(bootstrap_node.bootstrapped(None).await);

  // loop {
  //   let router_state = bootstrap_node.get_state().await.unwrap();
  //   println!("router state: {:#?}", router_state);
  //   tokio::time::sleep(Duration::from_secs(3)).await;
  // }

  // Start node A
  let a_socket = UdpSocket::bind(localhost(addr_family)).await.unwrap();
  log::info!("node a address: {}", a_socket.local_addr().unwrap());

  let a_addr = a_socket.local_addr().unwrap();
  let a_node = MainlineDht::builder()
    .add_node(bootstrap_node_addr)
    .set_read_only(false)
    .start("a_node", a_socket)
    .unwrap();

  // Start node B
  let b_socket = UdpSocket::bind(localhost(addr_family)).await.unwrap();
  log::info!("node b address: {}", b_socket.local_addr().unwrap());

  let b_node = MainlineDht::builder()
    .add_node(bootstrap_node_addr)
    .set_read_only(false)
    .start("b_node", b_socket)
    .unwrap();

  // Wait for both nodes to bootstrap
  assert!(a_node.bootstrapped(None).await);
  assert!(b_node.bootstrapped(None).await);

  // let router_state = bootstrap_node.get_state().await.unwrap();
  // let a_node_state = a_node.get_state().await.unwrap();
  // let b_node_state = b_node.get_state().await.unwrap();

  // println!(
  //   "router:\n{:?}\na_node:\n{:?}\nb_node:\n{:?}",
  //   router_state, a_node_state, b_node_state
  // );

  // let router_nodes = bootstrap_node.get_nodes().await.unwrap();
  // let a_node_nodes = a_node.get_nodes().await.unwrap();
  // let b_node_nodes = b_node.get_nodes().await.unwrap();

  // println!(
  //   "router:\n{:?}\na_node:\n{:?}\nb_node:\n{:?}",
  //   router_nodes, a_node_nodes, b_node_nodes
  // );

  let the_info_hash = InfoHash::sha1(b"foo");

  // Perform a lookup with announce by A. It should not return any peers initially but it should
  // make the network aware that A has the info_hash.
  let mut search = a_node.search(the_info_hash, true);
  assert_eq!(search.next().await, None);

  // // Now perform the lookup by B. It should find A.
  let mut search = b_node.search(the_info_hash, false);
  assert_eq!(search.next().await, Some(a_addr));

  // tokio::time::sleep(Duration::from_secs(6)).await;
  loop {}
}

#[derive(Copy, Clone)]
enum AddrFamily {
  V4,
  // V6,
}

fn localhost(family: AddrFamily) -> SocketAddr {
  match family {
    AddrFamily::V4 => (Ipv4Addr::LOCALHOST, 0).into(),
    // AddrFamily::V6 => (Ipv6Addr::LOCALHOST, 0).into(),
  }
}
