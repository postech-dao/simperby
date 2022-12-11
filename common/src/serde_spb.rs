use serde::{de::DeserializeOwned, ser::Serialize};
use serde_json::Error;

pub fn to_string<T: Serialize>(t: &T) -> Result<String, Error> {
    serde_json::to_string_pretty(t)
}

pub fn from_str<T: DeserializeOwned>(s: &str) -> Result<T, Error> {
    serde_json::from_str(s)
}

pub fn to_vec<T: Serialize>(t: &T) -> Result<Vec<u8>, Error> {
    serde_json::to_vec_pretty(t)
}

pub fn from_slice<T: DeserializeOwned>(s: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(s)
}
