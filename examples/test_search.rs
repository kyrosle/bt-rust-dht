use bt_rust_dht::{InfoHash, MainlineDht};
use futures_util::StreamExt;
use std::net::{Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;

use pretty_assertions::assert_eq;

#[tokio::main]
async fn main() {
  pretty_env_logger::init();
  announce_and_lookup().await;
}

async fn announce_and_lookup() {
  // Start the router node for the other nodes to bootstrap against.
  let (bootstrap_node, bootstrap_node_addr) =
    create_v4_node("router", None).await;

  assert!(bootstrap_node.bootstrapped(None).await);

  // ---------- //

  // Start node A
  let (a_node, a_addr) =
    create_v4_node("a_node", Some(vec![bootstrap_node_addr])).await;

  // Start node B
  let (b_node, b_addr) =
    create_v4_node("b_node", Some(vec![bootstrap_node_addr])).await;

  // Wait for both nodes to bootstrap
  assert!(a_node.bootstrapped(None).await);
  // panic!();
  assert!(b_node.bootstrapped(None).await);

  // ---------- //

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

  let info_hash_1 = InfoHash::sha1(b"foo");

  // Perform a lookup with announce by A. It should not return any peers initially but it should
  // make the network aware that A has the info_hash.
  log::info!("Assert a node announce the 'foo'.");
  let mut search = a_node.search(info_hash_1, true);
  assert_eq!(search.next().await, None);

  // // Now perform the lookup by B. It should find A.
  let mut search = b_node.search(info_hash_1, false);
  log::info!("Assert b node search the 'foo' from a node.");
  assert_eq!(search.next().await, Some(a_addr));

  let info_hash_2 = InfoHash::sha1(b"bob");

  let mut search = b_node.search(info_hash_2, true);
  log::info!("Assert b node announce the 'bob'.");
  assert_eq!(search.next().await, None);

  let mut search = a_node.search(info_hash_2, false);
  log::info!("Assert a node search the 'foo' from b node.");
  assert_eq!(search.next().await, Some(b_addr));

  // create a new node c and start it.
  let (c_node, c_addr) =
    create_v4_node("c_node", Some(vec![bootstrap_node_addr])).await;
  assert!(c_node.bootstrapped(None).await);

  let mut search = c_node.search(info_hash_1, false);
  log::info!("Assert c node search the 'foo' from a node and b node.");
  while let Some(node_addr) = search.next().await {
    assert!(vec![a_addr, b_addr].contains(&node_addr));
  }

  let (d_node, _d_addr) = create_v4_node("d_node", Some(vec![c_addr])).await;
  let mut search = d_node.search(info_hash_2, false);
  log::info!("Assert d node search 'foo' and get nothing because the bootstrap node is c which doesn't announce itself in this dht network");
  assert_eq!(search.next().await, None);
}

#[derive(Copy, Clone)]
enum AddrFamily {
  V4,
}

fn localhost(family: AddrFamily) -> SocketAddr {
  match family {
    AddrFamily::V4 => (Ipv4Addr::LOCALHOST, 0).into(),
  }
}

async fn create_v4_node(
  name: &str,
  bootstrap_nodes: Option<Vec<SocketAddr>>,
) -> (MainlineDht, SocketAddr) {
  // bind the random socket.
  let socket = UdpSocket::bind(localhost(AddrFamily::V4)).await.unwrap();
  let socket_addr = socket.local_addr().unwrap();

  log::info!("[{}] node address: {}", name, socket_addr);

  // build the node.
  let mut dht = MainlineDht::builder();
  if let Some(nodes) = bootstrap_nodes {
    dht = nodes.iter().fold(dht, |dht, node| dht.add_node(*node));
  }

  // startup the node.
  let dht = dht.set_read_only(false).start(name, socket).unwrap();

  (dht, socket_addr)
}
