use trust_dns_resolver::{
  config::{ResolverConfig, ResolverOpts},
  Resolver,
};

pub fn main() {
  let router_address = "router.bittorrent.com";

  println!("google:");
  let resolve =
    Resolver::new(ResolverConfig::google(), ResolverOpts::default()).unwrap();
  let response = resolve.lookup_ip(router_address).unwrap();

  for addr in response {
    println!("{:?}", addr);
  }

  println!(" ------------ ");
  println!(" ------------ ");

  println!("cloudflare:");
  let resolve =
    Resolver::new(ResolverConfig::cloudflare(), ResolverOpts::default())
      .unwrap();
  let response = resolve.lookup_ip(router_address).unwrap();

  for addr in response {
    println!("{:?}", addr);
  }
  println!(" ------------ ");

  println!("cloudflare tls:");
  let resolve_tls =
    Resolver::new(ResolverConfig::cloudflare_tls(), ResolverOpts::default())
      .unwrap();
  let response = resolve_tls.lookup_ip(router_address).unwrap();

  for addr in response {
    println!("{:?}", addr);
  }
  println!(" ------------ ");

  println!("cloudflare https:");
  let resolve_https =
    Resolver::new(ResolverConfig::cloudflare_https(), ResolverOpts::default())
      .unwrap();
  let response = resolve_https.lookup_ip(router_address).unwrap();

  for addr in response {
    println!("{:?}", addr);
  }
  println!(" ------------ ");
  println!(" ------------ ");

  println!("quad9:");
  let resolve_https =
    Resolver::new(ResolverConfig::quad9(), ResolverOpts::default()).unwrap();
  let response = resolve_https.lookup_ip(router_address).unwrap();

  for addr in response {
    println!("{:?}", addr);
  }
  println!(" ------------ ");

  println!("quad9 tls:");
  let resolve_https =
    Resolver::new(ResolverConfig::quad9_tls(), ResolverOpts::default())
      .unwrap();
  let response = resolve_https.lookup_ip(router_address).unwrap();

  for addr in response {
    println!("{:?}", addr);
  }
  println!(" ------------ ");

  println!("quad9 https:");
  let resolve_https =
    Resolver::new(ResolverConfig::quad9_https(), ResolverOpts::default())
      .unwrap();
  let response = resolve_https.lookup_ip(router_address).unwrap();

  for addr in response {
    println!("{:?}", addr);
  }
  println!(" ------------ ");
}
