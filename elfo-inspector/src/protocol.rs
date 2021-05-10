//! Types to use for cross-task communication.

use std::{fmt::Debug, net::IpAddr, str::FromStr, time::Duration};

use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};
use smartstring::alias::String;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use elfo::{config::Secret, Local};
use elfo_core as elfo;
use elfo_macros::message;

use crate::{
    api::TopologyUpdated,
    values::{ip_addr, InspectorError, UpdateError},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Config {
    #[serde(with = "ip_addr")]
    pub ip: IpAddr,
    pub port: u16,
    pub auth_tokens: Vec<Token>,
    #[serde(with = "humantime_serde")]
    pub update_period: Duration,
    #[serde(default = "parallel_requests_max_default")]
    pub parallel_requests_max: usize,
}

pub trait Request: elfo::Message
where
    Self::Update: Debug + Serialize,
{
    type Update;

    fn tx(&self) -> &Sender<Result<Self::Update, UpdateError>>;
}

#[message(elfo = elfo_core)]
pub(crate) struct GetTopology {
    pub meta: RequestMeta<TopologyUpdated>,
}

/// `U` — update type.
#[message(part, elfo = elfo_core)]
pub(crate) struct RequestMeta<U> {
    id: RequestId,
    token: Token,
    tx: Local<Sender<Result<U, UpdateError>>>,
}

#[message(part, elfo = elfo_core)]
pub(crate) struct RequestId(u64);

#[message(part, elfo = elfo_core)]
pub(crate) struct Token(Secret<String>);

const CHANNEL_SIZE: usize = 1024;

fn parallel_requests_max_default() -> usize {
    1024
}

impl GetTopology {
    pub fn new(
        token: Token,
    ) -> (
        Self,
        Receiver<Result<<Self as Request>::Update, UpdateError>>,
    ) {
        let (tx, rx) = channel(CHANNEL_SIZE);
        (
            Self {
                meta: RequestMeta::new(token, tx),
            },
            rx,
        )
    }
}

impl<U> RequestMeta<U>
where
    U: Serialize,
{
    fn new(token: Token, tx: Sender<Result<U, UpdateError>>) -> Self {
        Self {
            id: Default::default(),
            token,
            tx: Local::new(tx),
        }
    }
}

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
            Err(InspectorError::new("token is too short, should be at least 8 chars long").into())
        } else {
            Ok(Self(String::from(s).into()))
        }
    }
}

macro_rules! impl_request {
    ($( $t:tt ( update = $update:ty ) ),+ $(,)?) => {
        $(
            impl Request for $t {
                type Update = $update;

                fn tx(&self) -> &Sender<Result<Self::Update, UpdateError>> {
                    &*self.meta.tx
                }
            }
        )*
    }
}

impl_request!(GetTopology(update = TopologyUpdated));
