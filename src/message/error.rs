use std::fmt;

use serde::{
  de::{Error as _, IgnoredAny, Visitor},
  ser::SerializeSeq,
  Deserialize, Serialize,
};

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Error {
  pub code: u8,
  pub message: String,
}

impl Serialize for Error {
  fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let mut seq = s.serialize_seq(Some(2))?;
    seq.serialize_element(&self.code)?;
    seq.serialize_element(&self.message)?;
    seq.end()
  }
}

impl<'de> Deserialize<'de> for Error {
  fn deserialize<D>(d: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    struct ErrorVisitor;

    impl<'de> Visitor<'de> for ErrorVisitor {
      type Value = Error;

      fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a list of two elements: an integer and a string")
      }

      fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
      where
        A: serde::de::SeqAccess<'de>,
      {
        let code: u8 = seq
          .next_element()?
          .ok_or_else(|| A::Error::invalid_length(0, &self))?;

        let message = seq
          .next_element()?
          .ok_or_else(|| A::Error::invalid_length(1, &self))?;

        if seq.next_element::<IgnoredAny>()?.is_some() {
          return Err(A::Error::invalid_length(3, &self));
        }

        Ok(Error { code, message })
      }
    }

    d.deserialize_seq(ErrorVisitor)
  }
}

/// The following table describes the possible error codes:
pub mod error_code {
  // some of these codes are not used in this crate but we still list them here for completeness.
  #![allow(unused)]

  pub const GENERIC_ERROR: u8 = 201;
  pub const SERVER_ERROR: u8 = 202;
  pub const PROTOCOL_ERROR: u8 = 203;
  pub const METHOD_UNKNOWN: u8 = 204;
}
