use bt_rust_dht::router;
use tokio::net::UdpSocket;

// Testing the dht network in the local node bootstrapping stage.
//
// Here we can do some testing from sending the find_node command to the
// bootstrap node / bootstrap router.
// And then we check the response where we received or contain some relevant nodes.
#[tokio::main]
async fn main() {
  pretty_env_logger::init();

  let bootstrap_router = router::UTORRENT_DHT;
  let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
  let socket_addr = socket.local_addr().unwrap();
  log::info!("Starting listening on {}.", socket_addr);

  let router_address = tokio::net::lookup_host(bootstrap_router)
    .await
    .unwrap()
    .collect::<Vec<_>>();

  log::debug!(
    "lookup the {} host, and then find these nodes: \n{:#?}",
    bootstrap_router,
    router_address
  );
}
