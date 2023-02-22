use serde::{Deserialize, Serialize};

pub const ID_LEN: usize = 20;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Id(#[serde(with = "byte_array")] [u8; ID_LEN]);

mod byte_array {
    use super::ID_LEN;
}
