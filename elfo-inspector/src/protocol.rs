//! Types to use for cross-task communication.

use std::{fmt::Debug, net::IpAddr, str::FromStr, time::Duration};

use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};
use smartstring::alias::String;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use elfo::{config::Secret, Local, Topology};
use elfo_core as elfo;
use elfo_macros::message;

use crate::{
    api::{Update, UpdateResult},
    values::{ip_addr, InspectorError, TopologyActorGroup, TopologyConnection, UpdateError},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Config {
    #[serde(with = "ip_addr")]
    pub ip: IpAddr,
    pub port: u16,
    pub auth_tokens: Vec<Token>,
    #[serde(with = "humantime_serde", default = "heartbeat_period_default")]
    pub heartbeat_period: Duration,
    #[serde(with = "humantime_serde")]
    pub update_period: Duration,
    #[serde(default = "parallel_requests_max_default")]
    pub parallel_requests_max: usize,
}

#[message(elfo = elfo_core)]
pub(crate) struct Request {
    token: Token,
    tx: Local<Sender<UpdateResult>>,
    pub(crate) body: RequestBody,
}

#[message(elfo = elfo_core)]
pub(crate) struct HeartbeatTick;

#[message(elfo = elfo_core)]
pub(crate) struct TopologyUpdated {
    groups: Vec<TopologyActorGroup>,
    connections: Vec<TopologyConnection>,
}

#[message(part, elfo = elfo_core)]
#[derive(Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum RequestBody {
    GetTopology,
}

#[message(part, elfo = elfo_core)]
pub(crate) struct RequestId(u64);

#[message(part, elfo = elfo_core)]
pub(crate) struct Token(Secret<String>);

const CHANNEL_SIZE: usize = 1024;

fn heartbeat_period_default() -> Duration {
    Duration::from_secs(2)
}

fn parallel_requests_max_default() -> usize {
    1024
}

impl Request {
    pub(crate) fn new(token: Token, body: RequestBody) -> (Self, Receiver<UpdateResult>) {
        let (tx, rx) = channel(CHANNEL_SIZE);
        (
            Self {
                body,
                token,
                tx: Local::new(tx),
            },
            rx,
        )
    }

    pub(crate) fn tx(&self) -> &Sender<UpdateResult> {
        &*self.tx
    }
}

// impl PartialOrd for RequestBody {
// fn cmp(&self, other: &Self) -> cmp::Cmp {
// cmp::Equal
//}

impl Default for RequestId {
    fn default() -> Self {
        Self(1)
    }
}

impl FromStr for Token {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const LENGTH_MIN: usize = 8;
        if s.len() < LENGTH_MIN {
            Err(InspectorError::new("the string is too short to be a token").into())
        } else {
            Ok(Self(String::from(s).into()))
        }
    }
}

// impl From<Topology> for Update {
// fn from(topology: Topology) -> Self {
// let groups = topology.actor_groups().map(Into::into).collect();
// let connections = topology.connections().map(Into::into).collect();
// Self::TopologyUpdated {
// groups,
// connections,
//}
