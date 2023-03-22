use bt_rust_dht::{InfoHash, MainlineDht};
use futures_util::StreamExt;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use tokio::net::UdpSocket;

use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread")]
async fn announce_and_lookup_v4() {
  announce_and_lookup(AddrFamily::V4).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn announce_and_lookup_v6() {
  announce_and_lookup(AddrFamily::V6).await;
}

async fn announce_and_lookup(addr_family: AddrFamily) {
  // Start the router node for the other nodes to bootstrap against.
  let bootstrap_node_socket =
    UdpSocket::bind(localhost(addr_family)).await.unwrap();
  dbg!(
    "router node address: {}",
    bootstrap_node_socket.local_addr().unwrap()
  );

  let bootstrap_node_addr = bootstrap_node_socket.local_addr().unwrap();
  let bootstrap_node = MainlineDht::builder()
    .set_read_only(false)
    .start("router", bootstrap_node_socket)
    .unwrap();

  assert!(bootstrap_node.bootstrapped(None).await);

  // Start node A
  let a_socket = UdpSocket::bind(localhost(addr_family)).await.unwrap();

  let a_addr = a_socket.local_addr().unwrap();
  let a_node = MainlineDht::builder()
    .add_node(bootstrap_node_addr)
    .set_read_only(false)
    .start("a_node", a_socket)
    .unwrap();

  // Start node B
  let b_socket = UdpSocket::bind(localhost(addr_family)).await.unwrap();

  let b_addr = b_socket.local_addr().unwrap();
  let b_node = MainlineDht::builder()
    .add_node(bootstrap_node_addr)
    .set_read_only(false)
    .start("b_node", b_socket)
    .unwrap();

  // Wait for both nodes to bootstrap
  assert!(a_node.bootstrapped(None).await);
  assert!(b_node.bootstrapped(None).await);

  let the_info_hash = InfoHash::sha1(b"foo");

  // Perform a lookup with announce by A. It should not return any peers initially but it should
  // make the network aware that A has the info_hash.
  let mut search = a_node.search(the_info_hash, true);
  assert_eq!(search.next().await, None);

  // Now perform the lookup by B. It should find A.
  let mut search = b_node.search(the_info_hash, false);
  assert_eq!(search.next().await, Some(a_addr));

  let the_info_hash = InfoHash::sha1(b"312312321i3o13io2j");

  let mut search = b_node.search(the_info_hash, true);
  assert_eq!(search.next().await, None);

  let mut search = a_node.search(the_info_hash, false);
  assert_eq!(search.next().await, Some(b_addr));
}

#[derive(Copy, Clone)]
enum AddrFamily {
  V4,
  V6,
}

fn localhost(family: AddrFamily) -> SocketAddr {
  match family {
    AddrFamily::V4 => (Ipv4Addr::LOCALHOST, 0).into(),
    AddrFamily::V6 => (Ipv6Addr::LOCALHOST, 0).into(),
  }
}
