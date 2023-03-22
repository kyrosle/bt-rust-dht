use std::{net::SocketAddr, time::Duration};

use bt_rust_dht::{
  message::{FindNodeRequest, Message, MessageBody, Request, Want},
  router,
  transaction::AIDGenerator,
};
use tokio::net::UdpSocket;

// Testing the dht network in the local node bootstrapping stage.
//
// Here we can do some testing from sending the find_node command to the
// bootstrap node / bootstrap router.
// And then we check the response where we received or contain some relevant nodes.
#[tokio::main]
async fn main() {
  pretty_env_logger::init();

  let bootstrap_router = router::BITTORRENT_DHT;
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

  log::info!("Checking the nodes whether we can contract with?");

  let mut aid_generator = AIDGenerator::default();
  let mut mid_generator = aid_generator.generate();

  let trans_id = mid_generator.generate();

  let table_id = rand::random();

  let message = Message {
    transaction_id: trans_id.as_ref().to_vec(),
    body: MessageBody::Request(Request::FindNode(FindNodeRequest {
      id: table_id,
      target: table_id,
      want: Some(Want::V4),
    })),
  }
  .encode();

  for addr in router_address.iter() {
    if addr.is_ipv6() {
      continue;
    }
    match socket.connect(addr).await {
      Ok(_) => {
        log::info!("Connecting success {}.", addr);
        socket.send(&message).await.unwrap();
      }
      Err(_) => log::error!("Connect failure {}", addr),
    }
  }

  tokio::task::spawn(async move { recv_response(socket).await });
  // tokio::task::spawn(async move { assert_recv_startup(socket_addr).await });

  std::thread::sleep(Duration::from_secs(20));
}

async fn assert_recv_startup(addr: SocketAddr) {
  let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
  let socket_addr = socket.local_addr().unwrap();
  log::info!("Startup the interval sending node: {}", socket_addr);

  socket.connect(addr).await.unwrap();

  let message = "Hello handshake!";
  loop {
    socket.send(message.as_bytes()).await.unwrap();

    tokio::time::sleep(Duration::from_secs(3)).await;
  }
}

async fn recv_response(socket: UdpSocket) {
  let mut buffer = [0u8; 128];
  while let Ok(size) = socket.recv(&mut buffer).await {
    let buffer = buffer.iter().copied().take_while(|u| u != &0).collect();
    let message = String::from_utf8(buffer).unwrap();
    log::info!("recv response {} bytes.\n{:?}", size, message);
  }
}
