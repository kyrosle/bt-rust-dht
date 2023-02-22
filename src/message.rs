use serde::{Deserialize, Serialize};

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "t", with = "serde_bytes")]
    pub transaction_id: Vec<u8>,
    #[serde(flatten)]
    pub body: MessageBody,
}

pub enum MessageBody {
  Request(Request),
  Response(Response),
  Error(Error)
}

pub enum Request {
  Ping(PingRequest),
  FindNode(FindNodeRequest),
  GetPeers(GetPeersRequest),
  AnnouncePeer(AnnouncePeerRequest),
}

pub struct PingRequest {
  pub id: NodeId,
}