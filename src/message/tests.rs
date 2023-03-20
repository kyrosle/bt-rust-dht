#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;
  use std::net::{Ipv4Addr, Ipv6Addr};

  #[test]
  fn serialize_ping_request() {
    let encoded = "d1:ad2:id20:abcdefghij0123456789e1:q4:ping1:t2:aa1:y1:qe";
    let decoded = Message {
      transaction_id: b"aa".to_vec(),
      body: MessageBody::Request(Request::Ping(PingRequest {
        id: NodeId::from(*b"abcdefghij0123456789"),
      })),
    };

    assert_serialize_deserialize(encoded, &decoded)
  }

  #[test]
  fn serialize_find_node_request() {
    let encoded = "d1:ad2:id20:abcdefghij01234567896:target20:mnopqrstuvwxyz123456e1:q9:find_node1:t2:aa1:y1:qe";
    let decoded = Message {
      transaction_id: b"aa".to_vec(),
      body: MessageBody::Request(Request::FindNode(FindNodeRequest {
        id: NodeId::from(*b"abcdefghij0123456789"),
        target: NodeId::from(*b"mnopqrstuvwxyz123456"),
        want: None,
      })),
    };

    assert_serialize_deserialize(encoded, &decoded)
  }

  #[test]
  fn serialize_find_node_request_with_want() {
    let encoded = "d1:ad2:id20:abcdefghij01234567896:target20:mnopqrstuvwxyz1234564:wantl2:n42:n6ee1:q9:find_node1:t2:aa1:y1:qe";
    let decoded = Message {
      transaction_id: b"aa".to_vec(),
      body: MessageBody::Request(Request::FindNode(FindNodeRequest {
        id: NodeId::from(*b"abcdefghij0123456789"),
        target: NodeId::from(*b"mnopqrstuvwxyz123456"),
        want: Some(Want::Both),
      })),
    };

    assert_serialize_deserialize(encoded, &decoded)
  }

  #[test]
  fn serialize_get_peers_request() {
    let encoded = "d1:ad2:id20:abcdefghij01234567899:info_hash20:mnopqrstuvwxyz123456e1:q9:get_peers1:t2:aa1:y1:qe";
    let decoded = Message {
      transaction_id: b"aa".to_vec(),
      body: MessageBody::Request(Request::GetPeers(GetPeersRequest {
        id: NodeId::from(*b"abcdefghij0123456789"),
        info_hash: InfoHash::from(*b"mnopqrstuvwxyz123456"),
        want: None,
      })),
    };

    assert_serialize_deserialize(encoded, &decoded)
  }

  #[test]
  fn serialize_get_peers_request_with_want() {
    let encoded = "d1:ad2:id20:abcdefghij01234567899:info_hash20:mnopqrstuvwxyz1234564:wantl2:n4ee1:q9:get_peers1:t2:aa1:y1:qe";
    let decoded = Message {
      transaction_id: b"aa".to_vec(),
      body: MessageBody::Request(Request::GetPeers(GetPeersRequest {
        id: NodeId::from(*b"abcdefghij0123456789"),
        info_hash: InfoHash::from(*b"mnopqrstuvwxyz123456"),
        want: Some(Want::V4),
      })),
    };

    assert_serialize_deserialize(encoded, &decoded)
  }

  #[test]
  fn serialize_announce_peer_request_with_implied_port() {
    let encoded = "d1:ad2:id20:abcdefghij012345678912:implied_porti1e9:info_hash20:mnopqrstuvwxyz1234565:token8:aoeusnthe1:q13:announce_peer1:t2:aa1:y1:qe";
    let decoded = Message {
      transaction_id: b"aa".to_vec(),
      body: MessageBody::Request(Request::AnnouncePeer(AnnouncePeerRequest {
        id: NodeId::from(*b"abcdefghij0123456789"),
        port: None,
        info_hash: InfoHash::from(*b"mnopqrstuvwxyz123456"),
        token: b"aoeusnth".to_vec(),
      })),
    };

    assert_serialize_deserialize(encoded, &decoded);
  }

  #[test]
  fn serialize_announce_peer_request_with_explicit_port() {
    let encoded = "d1:ad2:id20:abcdefghij01234567899:info_hash20:mnopqrstuvwxyz1234564:porti6881e5:token8:aoeusnthe1:q13:announce_peer1:t2:aa1:y1:qe";
    let decoded = Message {
      transaction_id: b"aa".to_vec(),
      body: MessageBody::Request(Request::AnnouncePeer(AnnouncePeerRequest {
        id: NodeId::from(*b"abcdefghij0123456789"),
        port: Some(6881),
        info_hash: InfoHash::from(*b"mnopqrstuvwxyz123456"),
        token: b"aoeusnth".to_vec(),
      })),
    };

    assert_serialize_deserialize(encoded, &decoded);
  }

  #[test]
  fn serialize_other_response_none() {
    let encoded = "d1:rd2:id20:mnopqrstuvwxyz123456e1:t2:aa1:y1:re";
    let decoded = Message {
      transaction_id: b"aa".to_vec(),
      body: MessageBody::Response(Response {
        id: NodeId::from(*b"mnopqrstuvwxyz123456"),
        values: vec![],
        nodes_v4: vec![],
        nodes_v6: vec![],
        token: None,
      }),
    };

    assert_serialize_deserialize(encoded, &decoded);
  }

  #[test]
  fn serialize_other_response_v4() {
    let encoded =
            "d1:rd2:id20:0123456789abcdefghij5:nodes26:mnopqrstuvwxyz012345axje.ue1:t2:aa1:y1:re";
    let decoded = Message {
      transaction_id: b"aa".to_vec(),
      body: MessageBody::Response(Response {
        id: NodeId::from(*b"0123456789abcdefghij"),
        values: vec![],
        nodes_v4: vec![NodeHandle {
          id: NodeId::from(*b"mnopqrstuvwxyz012345"),
          addr: (Ipv4Addr::new(97, 120, 106, 101), 11893).into(),
        }],
        nodes_v6: vec![],
        token: None,
      }),
    };

    assert_serialize_deserialize(encoded, &decoded);
  }

  #[test]
  fn serialize_other_response_v6() {
    let encoded =
            "d1:rd2:id20:0123456789abcdefghij6:nodes638:mnopqrstuvwxyz012345abcdefghijklmnop.ue1:t2:aa1:y1:re";
    let decoded = Message {
      transaction_id: b"aa".to_vec(),
      body: MessageBody::Response(Response {
        id: NodeId::from(*b"0123456789abcdefghij"),
        values: vec![],
        nodes_v4: vec![],
        nodes_v6: vec![NodeHandle {
          id: NodeId::from(*b"mnopqrstuvwxyz012345"),
          addr: (
            Ipv6Addr::new(
              0x6162, 0x6364, 0x6566, 0x6768, 0x696a, 0x6b6c, 0x6d6e, 0x6f70,
            ),
            11893,
          )
            .into(),
        }],
        token: None,
      }),
    };

    assert_serialize_deserialize(encoded, &decoded);
  }

  #[test]
  fn serialize_other_response_both() {
    let encoded =
            "d1:rd2:id20:0123456789abcdefghij5:nodes26:mnopqrstuvwxyz012345axje.u6:nodes638:6789abcdefghijklmnopabcdefghijklmnop.ue1:t2:aa1:y1:re";
    let decoded = Message {
      transaction_id: b"aa".to_vec(),
      body: MessageBody::Response(Response {
        id: NodeId::from(*b"0123456789abcdefghij"),
        values: vec![],
        nodes_v4: vec![NodeHandle {
          id: NodeId::from(*b"mnopqrstuvwxyz012345"),
          addr: (Ipv4Addr::new(97, 120, 106, 101), 11893).into(),
        }],
        nodes_v6: vec![NodeHandle {
          id: NodeId::from(*b"6789abcdefghijklmnop"),
          addr: (
            Ipv6Addr::new(
              0x6162, 0x6364, 0x6566, 0x6768, 0x696a, 0x6b6c, 0x6d6e, 0x6f70,
            ),
            11893,
          )
            .into(),
        }],
        token: None,
      }),
    };

    assert_serialize_deserialize(encoded, &decoded);
  }

  #[test]
  fn serialize_get_peers_response_with_values() {
    let encoded = "d1:rd2:id20:abcdefghij01234567895:token8:aoeusnth6:valuesl6:axje.u6:idhtnmee1:t2:aa1:y1:re";
    let decoded = Message {
      transaction_id: b"aa".to_vec(),
      body: MessageBody::Response(Response {
        id: NodeId::from(*b"abcdefghij0123456789"),
        values: vec![
          (Ipv4Addr::new(97, 120, 106, 101), 11893).into(),
          (Ipv4Addr::new(105, 100, 104, 116), 28269).into(),
        ],
        nodes_v4: vec![],
        nodes_v6: vec![],
        token: Some(b"aoeusnth".to_vec()),
      }),
    };

    assert_serialize_deserialize(encoded, &decoded);
  }

  #[test]
  fn serialize_get_peers_response_with_nodes_v4() {
    let encoded =
            "d1:rd2:id20:abcdefghij01234567895:nodes52:mnopqrstuvwxyz123456axje.u789abcdefghijklmnopqidhtnm5:token8:aoeusnthe1:t2:aa1:y1:re";
    let decoded = Message {
      transaction_id: b"aa".to_vec(),
      body: MessageBody::Response(Response {
        id: NodeId::from(*b"abcdefghij0123456789"),
        values: vec![],
        nodes_v4: vec![
          NodeHandle {
            id: NodeId::from(*b"mnopqrstuvwxyz123456"),
            addr: (Ipv4Addr::new(97, 120, 106, 101), 11893).into(),
          },
          NodeHandle {
            id: NodeId::from(*b"789abcdefghijklmnopq"),
            addr: (Ipv4Addr::new(105, 100, 104, 116), 28269).into(),
          },
        ],
        nodes_v6: vec![],
        token: Some(b"aoeusnth".to_vec()),
      }),
    };

    assert_serialize_deserialize(encoded, &decoded);
  }

  #[test]
  fn serialize_error() {
    let encoded = "d1:eli201e23:A Generic Error Ocurrede1:t2:aa1:y1:ee";
    let decoded = Message {
      transaction_id: b"aa".to_vec(),
      body: MessageBody::Error(Error {
        code: error_code::GENERIC_ERROR,
        message: "A Generic Error Ocurred".to_owned(),
      }),
    };

    assert_serialize_deserialize(encoded, &decoded);
  }

  #[track_caller]
  fn assert_serialize_deserialize(encoded: &str, decoded: &Message) {
    let l_encoded = serde_bencoded::to_string(decoded).unwrap();
    assert_eq!(l_encoded, encoded);
    let r_decoded = Message::decode(encoded.as_bytes()).unwrap();
    assert_eq!(r_decoded, *decoded);
  }
}
