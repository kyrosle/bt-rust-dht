
use std::{fmt, ops::BitXor};

use rand::{distributions::Standard, prelude::Distribution};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const ID_LEN: usize = 20;

#[derive(
  Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
// Node IDs are chosen at random from the same 160-bit space as BitTorrent infohashes.
pub struct Id(#[serde(with = "byte_array")] [u8; ID_LEN]);

impl AsRef<[u8]> for Id {
  fn as_ref(&self) -> &[u8] {
    &self.0
  }
}

impl From<Id> for [u8; ID_LEN] {
  fn from(hash: Id) -> Self {
    hash.0
  }
}

impl From<[u8; ID_LEN]> for Id {
  fn from(value: [u8; ID_LEN]) -> Self {
    Self(value)
  }
}

#[derive(Debug, Error)]
#[error("invalid id length")]
pub struct LengthError;

impl<'a> TryFrom<&'a [u8]> for Id {
  type Error = LengthError;
  fn try_from(slice: &'a [u8]) -> Result<Self, Self::Error> {
    Ok(Id(slice.try_into().map_err(|_| LengthError)?))
  }
}

impl BitXor for Id {
  type Output = Self;

  fn bitxor(mut self, rhs: Self) -> Self::Output {
    for (src, dst) in rhs.0.iter().zip(self.0.iter_mut()) {
      *dst ^= *src;
    }
    self
  }
}

/// Used to create a random instance of `Id`.
impl Distribution<Id> for Standard {
  fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Id {
    Id(rng.gen())
  }
}

/// Format output in number hexadecimal.
impl fmt::LowerHex for Id {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for b in &self.0 {
      write!(f, "{:02x}", b)?;
    }
    Ok(())
  }
}

impl fmt::Debug for Id {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:x}", self)
  }
}

/// Helper to deserialize the `Id`
mod byte_array {
  use serde::{
    de::{Deserialize, Deserializer, Error},
    ser::{Serialize, Serializer},
  };
  use serde_bytes::{ByteBuf, Bytes};

  use super::ID_LEN;

  pub(super) fn serialize<S: Serializer>(
    bytes: &[u8; ID_LEN],
    s: S,
  ) -> Result<S::Ok, S::Error> {
    Bytes::new(bytes.as_ref()).serialize(s)
  }

  pub(super) fn deserialize<'de, D: Deserializer<'de>>(
    d: D,
  ) -> Result<[u8; ID_LEN], D::Error> {
    let buf = ByteBuf::deserialize(d)?;
    let buf = buf.into_vec();
    let len = buf.len();

    buf.try_into().map_err(|_| {
      let expected = format!("{}", ID_LEN);
      D::Error::invalid_length(len, &expected.as_ref())
    })
  }
}

/// BitTorrent `NodeId`.
pub type NodeId = Id;

/// BitTorrent `InfoHash`.
pub type InfoHash = Id;

/// Length of a `NodeId`.
pub const NODE_ID_LEN: usize = ID_LEN;

/// Length of a `InfoHash`.
pub const INFO_HASH_LEN: usize = ID_LEN;