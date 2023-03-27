use std::collections::HashSet;

use bt_rust_dht::{router, worker::resolve, IpVersion};
#[tokio::main]
async fn main() {
  let router = vec![router::UTORRENT_DHT, router::TRANSMISSION_DHT]
    .into_iter()
    .map(|s| s.to_string())
    .collect::<HashSet<_>>();

  let result = resolve(&router, IpVersion::V4).await;

  println!("{result:#?}");
}
