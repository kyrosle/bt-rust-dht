/// Helper to serialize/deserialize `Response`/`Error` in `MessageBody`
pub(super) mod unflatten {
  macro_rules! impl_unflatten {
    ($mod:ident, $field:literal) => {
      pub mod $mod {
        use serde::{Deserialize, Deserializer, Serialize, Serializer};

        #[derive(Serialize, Deserialize)]
        struct Wrapper<T> {
          #[serde(rename = $field)]
          field: T,
        }

        pub(crate) fn serialize<T: Serialize, S: Serializer>(
          value: &T,
          s: S,
        ) -> Result<S::Ok, S::Error> {
          Wrapper { field: value }.serialize(s)
        }

        pub(crate) fn deserialize<
          'de,
          T: Deserialize<'de>,
          D: Deserializer<'de>,
        >(
          d: D,
        ) -> Result<T, D::Error> {
          let wrapper = Wrapper::deserialize(d)?;
          Ok(wrapper.field)
        }
      }
    };
  }

  impl_unflatten!(response, "r");
  impl_unflatten!(error, "e");
}

#[derive(Clone, Eq, PartialEq, Debug, Copy)]
pub enum Want {
  // The peer wants only ipv4 contacts
  V4,
  // The peer wants only ipv6 contacts
  V6,
  // The peer wants both ipv4 and ipv6 contacts
  Both,
}

/// Helper to serialize or deserialize the enum `Want`
pub(super) mod want {
  use serde::{de::Visitor, ser::SerializeSeq, Deserializer, Serializer};
  use serde_bytes::Bytes;

  use super::Want;

  pub fn serialize<S: Serializer>(
    want: &Option<Want>,
    s: S,
  ) -> Result<S::Ok, S::Error> {
    let len = match want {
      None => 0,
      Some(Want::V4 | Want::V6) => 1,
      Some(Want::Both) => 2,
    };

    let mut seq = s.serialize_seq(Some(len))?;

    if matches!(want, Some(Want::V4 | Want::Both)) {
      seq.serialize_element(Bytes::new(b"n4"))?;
    }

    if matches!(want, Some(Want::V6 | Want::Both)) {
      seq.serialize_element(Bytes::new(b"n6"))?;
    }

    seq.end()
  }

  pub fn deserialize<'de, D>(d: D) -> Result<Option<Want>, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct WantVisitor;

    impl<'de> Visitor<'de> for WantVisitor {
      type Value = Option<Want>;

      fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "a list of strings")
      }

      fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
      where
        A: serde::de::SeqAccess<'de>,
      {
        let mut value = None;

        while let Some(s) = seq.next_element::<String>()? {
          value = match (value, s.as_str().trim()) {
            (None, "n4" | "N4") => Some(Want::V4),
            (None, "n6" | "N6") => Some(Want::V6),
            (Some(Want::V4), "n6" | "N6") => Some(Want::Both),
            (Some(Want::V6), "n4" | "N4") => Some(Want::Both),
            (_, _) => value,
          }
        }

        Ok(value)
      }
    }

    d.deserialize_seq(WantVisitor)
  }
}

/// Helper to serialize or deserialize the `port` field.
pub(super) mod port {
  use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};

  #[derive(Serialize, Deserialize)]
  struct Wrapper {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    port: Option<u16>,

    #[serde(
      default,
      skip_serializing_if = "is_false",
      deserialize_with = "deserialize_bool"
    )]
    implied_port: bool,
  }

  pub fn serialize<S: Serializer>(
    port: &Option<u16>,
    s: S,
  ) -> Result<S::Ok, S::Error> {
    Wrapper {
      implied_port: port.is_none(),
      port: *port,
    }
    .serialize(s)
  }

  pub fn deserialize<'de, D: Deserializer<'de>>(
    d: D,
  ) -> Result<Option<u16>, D::Error> {
    let wrapper = Wrapper::deserialize(d)?;

    if wrapper.implied_port {
      Ok(None)
    } else if wrapper.port.is_some() {
      Ok(wrapper.port)
    } else {
      Err(D::Error::missing_field("port"))
    }
  }

  fn is_false(b: &bool) -> bool {
    !*b
  }

  fn deserialize_bool<'de, D: Deserializer<'de>>(
    d: D,
  ) -> Result<bool, D::Error> {
    let num = u8::deserialize(d)?;
    Ok(num > 0)
  }
}
