use std::net::IpAddr;

use serde::{Deserialize, Deserializer, Serializer};

pub(crate) fn serialize<S>(ip: &IpAddr, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&ip.to_string())
}

pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<IpAddr, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}
