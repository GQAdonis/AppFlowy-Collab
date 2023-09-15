use std::cmp::Ordering;
use std::fmt::{Display, Formatter};

use bytes::Bytes;
use collab::core::origin::CollabOrigin;
use collab_define::CollabType;
use serde::{Deserialize, Serialize};

pub trait CollabSinkMessage: Clone + Send + Sync + 'static + Ord + Display {
  /// Returns the length of the message in bytes.
  fn length(&self) -> usize;
  /// Returns true if the message can be merged with other messages.
  /// Check the implementation of `queue_or_merge_msg` for more details.
  fn mergeable(&self) -> bool;

  fn merge(&mut self, other: Self);

  fn is_init_msg(&self) -> bool;

  /// Determine if the message can be deferred base on the current state of the sink.
  fn deferrable(&self) -> bool;
}

pub type MsgId = u64;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CollabMessage {
  ClientInit(ClientCollabInit),
  ServerSync(CollabServerSync),
  ClientUpdate(ClientCollabUpdate),
  AwarenessUpdate(CSAwarenessUpdate),
  ServerResponse(CollabServerResponse),
  ServerBroadcast(CollabServerBroadcast),
  ServerAck(CollabServerAck),
}

impl CollabSinkMessage for CollabMessage {
  fn length(&self) -> usize {
    self.payload().map(|p| p.len()).unwrap_or(0)
  }

  fn mergeable(&self) -> bool {
    false
  }

  fn merge(&mut self, _other: Self) {
    // Do nothing. Because mergeable is false.
  }

  fn is_init_msg(&self) -> bool {
    self.is_init()
  }

  fn deferrable(&self) -> bool {
    // If the message is not init, it can be pending.
    !self.is_init()
  }
}

impl Eq for CollabMessage {}

impl PartialEq for CollabMessage {
  fn eq(&self, other: &Self) -> bool {
    self.msg_id() == other.msg_id()
  }
}

impl PartialOrd for CollabMessage {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for CollabMessage {
  fn cmp(&self, other: &Self) -> Ordering {
    match (&self, &other) {
      (CollabMessage::ClientInit { .. }, CollabMessage::ClientInit { .. }) => Ordering::Equal,
      (CollabMessage::ClientInit { .. }, _) => Ordering::Greater,
      (_, CollabMessage::ClientInit { .. }) => Ordering::Less,
      (CollabMessage::ServerSync { .. }, CollabMessage::ServerSync { .. }) => Ordering::Equal,
      (CollabMessage::ServerSync { .. }, _) => Ordering::Greater,
      (_, CollabMessage::ServerSync { .. }) => Ordering::Less,
      _ => self.msg_id().cmp(&other.msg_id()).reverse(),
    }
  }
}

impl CollabMessage {
  /// Currently, only have one business id. So just return 1.
  pub fn business_id(&self) -> u8 {
    1
  }

  pub fn is_init(&self) -> bool {
    matches!(self, CollabMessage::ClientInit(_))
  }

  pub fn msg_id(&self) -> Option<MsgId> {
    match self {
      CollabMessage::ClientInit(value) => Some(value.msg_id),
      CollabMessage::ServerSync(value) => Some(value.msg_id),
      CollabMessage::ClientUpdate(value) => Some(value.msg_id),
      CollabMessage::ServerResponse(_) => None,
      CollabMessage::ServerBroadcast(_) => None,
      CollabMessage::AwarenessUpdate(_) => None,
      CollabMessage::ServerAck(value) => Some(value.msg_id),
    }
  }

  pub fn is_empty(&self) -> bool {
    match self {
      CollabMessage::ClientInit(value) => value.payload.is_empty(),
      CollabMessage::ServerSync(value) => value.payload.is_empty(),
      CollabMessage::ClientUpdate(value) => value.payload.is_empty(),
      CollabMessage::ServerResponse(value) => value.payload.is_empty(),
      CollabMessage::ServerBroadcast(value) => value.payload.is_empty(),
      CollabMessage::AwarenessUpdate(value) => value.payload.is_empty(),
      CollabMessage::ServerAck(value) => match value.payload {
        Some(ref payload) => payload.is_empty(),
        None => true,
      },
    }
  }

  pub fn origin(&self) -> Option<&CollabOrigin> {
    match self {
      CollabMessage::ClientInit(value) => Some(&value.origin),
      CollabMessage::ServerSync(value) => Some(&value.origin),
      CollabMessage::ClientUpdate(value) => Some(&value.origin),
      CollabMessage::ServerResponse(value) => value.origin.as_ref(),
      CollabMessage::ServerBroadcast(value) => Some(&value.origin),
      CollabMessage::AwarenessUpdate(_) => None,
      CollabMessage::ServerAck(_) => None,
    }
  }

  pub fn object_id(&self) -> &str {
    match self {
      CollabMessage::ClientInit(value) => &value.object_id,
      CollabMessage::ServerSync(value) => &value.object_id,
      CollabMessage::ClientUpdate(value) => &value.object_id,
      CollabMessage::ServerResponse(value) => &value.object_id,
      CollabMessage::ServerBroadcast(value) => &value.object_id,
      CollabMessage::AwarenessUpdate(value) => &value.object_id,
      CollabMessage::ServerAck(value) => &value.object_id,
    }
  }
}

impl Display for CollabMessage {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      CollabMessage::ClientInit(value) => f.write_fmt(format_args!(
        "client init: [{}|oid:{}|payload_len:{}|msg_id:{}]",
        value.origin,
        value.object_id,
        value.payload.len(),
        value.msg_id,
      )),
      CollabMessage::ServerSync(value) => f.write_fmt(format_args!(
        "server sync state: [oid:{}|payload_len:{}|msg_id:{}]",
        value.object_id,
        value.payload.len(),
        value.msg_id,
      )),
      CollabMessage::ClientUpdate(value) => f.write_fmt(format_args!(
        "send client update: [{}|oid:{}|msg_id:{}|payload_len:{}]",
        value.origin,
        value.object_id,
        value.msg_id,
        value.payload.len(),
      )),
      CollabMessage::ServerResponse(value) => f.write_fmt(format_args!(
        "server response: [oid:{}|payload_len:{}]",
        value.object_id,
        value.payload.len(),
      )),
      CollabMessage::ServerBroadcast(value) => f.write_fmt(format_args!(
        "broadcast update: [{}|oid:{}|payload_len:{}]",
        value.origin,
        value.object_id,
        value.payload.len(),
      )),
      CollabMessage::AwarenessUpdate(value) => f.write_fmt(format_args!(
        "awareness: [oid:{}|payload_len:{}]",
        value.object_id,
        value.payload.len(),
      )),
      CollabMessage::ServerAck(value) => f.write_fmt(format_args!(
        "ack message: [oid:{}|msg_id:{}]",
        value.object_id, value.msg_id,
      )),
    }
  }
}

impl From<CollabMessage> for Bytes {
  fn from(msg: CollabMessage) -> Self {
    Bytes::from(msg.to_vec())
  }
}

impl CollabMessage {
  pub fn to_vec(&self) -> Vec<u8> {
    serde_json::to_vec(self).unwrap_or_default()
  }

  pub fn from_vec(data: &[u8]) -> Result<Self, serde_json::Error> {
    serde_json::from_slice(data)
  }

  pub fn into_payload(self) -> Vec<u8> {
    match self {
      CollabMessage::ClientInit(value) => value.payload,
      CollabMessage::ServerSync(value) => value.payload,
      CollabMessage::ClientUpdate(value) => value.payload,
      CollabMessage::ServerResponse(value) => value.payload,
      CollabMessage::ServerBroadcast(value) => value.payload,
      CollabMessage::AwarenessUpdate(value) => value.payload,
      CollabMessage::ServerAck(value) => match value.payload {
        Some(payload) => payload,
        None => vec![],
      },
    }
  }

  pub fn payload(&self) -> Option<&Vec<u8>> {
    match self {
      CollabMessage::ClientInit(value) => Some(&value.payload),
      CollabMessage::ServerSync(value) => Some(&value.payload),
      CollabMessage::ClientUpdate(value) => Some(&value.payload),
      CollabMessage::ServerResponse(value) => Some(&value.payload),
      CollabMessage::ServerBroadcast(value) => Some(&value.payload),
      CollabMessage::AwarenessUpdate(value) => Some(&value.payload),
      CollabMessage::ServerAck(value) => value.payload.as_ref(),
    }
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct CSAwarenessUpdate {
  object_id: String,
  payload: Vec<u8>,
}

impl CSAwarenessUpdate {
  pub fn new(object_id: String, payload: Vec<u8>) -> Self {
    Self { object_id, payload }
  }
}

impl From<CSAwarenessUpdate> for CollabMessage {
  fn from(value: CSAwarenessUpdate) -> Self {
    CollabMessage::AwarenessUpdate(value)
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ClientCollabUpdate {
  origin: CollabOrigin,
  object_id: String,
  msg_id: MsgId,
  payload: Vec<u8>,
}

impl ClientCollabUpdate {
  pub fn new(origin: CollabOrigin, object_id: String, msg_id: MsgId, payload: Vec<u8>) -> Self {
    Self {
      origin,
      object_id,
      msg_id,
      payload,
    }
  }
}

impl From<ClientCollabUpdate> for CollabMessage {
  fn from(value: ClientCollabUpdate) -> Self {
    CollabMessage::ClientUpdate(value)
  }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CollabServerAck {
  pub object_id: String,
  pub msg_id: MsgId,
  pub payload: Option<Vec<u8>>,
}

impl CollabServerAck {
  pub fn new(object_id: String, msg_id: MsgId, payload: Option<Vec<u8>>) -> Self {
    Self {
      object_id,
      msg_id,
      payload,
    }
  }
}

impl From<CollabServerAck> for CollabMessage {
  fn from(value: CollabServerAck) -> Self {
    CollabMessage::ServerAck(value)
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ClientCollabInit {
  pub origin: CollabOrigin,
  pub object_id: String,
  pub collab_type: CollabType,
  pub workspace_id: String,
  pub msg_id: MsgId,
  pub payload: Vec<u8>,
  pub md5: String,
}

impl ClientCollabInit {
  pub fn new(
    origin: CollabOrigin,
    object_id: String,
    collab_type: CollabType,
    workspace_id: String,
    msg_id: MsgId,
    payload: Vec<u8>,
  ) -> Self {
    let md5 = md5(&payload);
    Self {
      origin,
      object_id,
      collab_type,
      workspace_id,
      msg_id,
      payload,
      md5,
    }
  }
}

impl From<ClientCollabInit> for CollabMessage {
  fn from(value: ClientCollabInit) -> Self {
    CollabMessage::ClientInit(value)
  }
}

pub fn md5<T: AsRef<[u8]>>(data: T) -> String {
  let digest = md5::compute(data);
  format!("{:x}", digest)
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct CollabServerResponse {
  origin: Option<CollabOrigin>,
  object_id: String,
  payload: Vec<u8>,
}

impl CollabServerResponse {
  pub fn new(origin: Option<CollabOrigin>, object_id: String, payload: Vec<u8>) -> Self {
    Self {
      origin,
      object_id,
      payload,
    }
  }
}

impl From<CollabServerResponse> for CollabMessage {
  fn from(value: CollabServerResponse) -> Self {
    CollabMessage::ServerResponse(value)
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct CollabServerBroadcast {
  origin: CollabOrigin,
  object_id: String,
  payload: Vec<u8>,
}

impl CollabServerBroadcast {
  pub fn new(origin: CollabOrigin, object_id: String, payload: Vec<u8>) -> Self {
    Self {
      origin,
      object_id,
      payload,
    }
  }
}

impl From<CollabServerBroadcast> for CollabMessage {
  fn from(value: CollabServerBroadcast) -> Self {
    CollabMessage::ServerBroadcast(value)
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct CollabServerSync {
  // Indicates the origin client of the message
  pub origin: CollabOrigin,
  pub object_id: String,
  pub payload: Vec<u8>,
  pub msg_id: MsgId,
}

impl CollabServerSync {
  pub fn new(origin: CollabOrigin, object_id: String, payload: Vec<u8>, msg_id: MsgId) -> Self {
    Self {
      origin,
      object_id,
      payload,
      msg_id,
    }
  }
}

impl From<CollabServerSync> for CollabMessage {
  fn from(value: CollabServerSync) -> Self {
    CollabMessage::ServerSync(value)
  }
}
