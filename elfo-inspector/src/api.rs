//! Types to use for communication with clients.
//! All endpoints can return 400, 401, 429 and 500.

use serde::Serialize;
use smartstring::alias::String;
use strum::AsRefStr;
use tokio::sync::mpsc::Sender;

use elfo::{
    topology::{ActorGroup, Connection},
    Topology,
};
use elfo_core as elfo;
use elfo_macros::message;

use crate::values::{Addr, TopologyActorGroup, TopologyConnection, UpdateError};

type Timestamp = f64;
pub(crate) type UpdateResult = Result<Update, UpdateError>;

#[message(part, elfo = elfo_core)]
#[serde(rename_all = "camelCase")]
#[derive(AsRefStr)]
pub(crate) enum Update {
    Heartbeat,
    /// SSE GET: /api/v1/topology
    TopologyUpdated {
        groups: Vec<TopologyActorGroup>,
        connections: Vec<TopologyConnection>,
    },
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum UpdateKey {
    Heartbeat,
    TopologyUpdated,
}

/// SSE GET: /api/v1/groups

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActorGroupUpdated {
    addr: Addr,
    status: elfo::ActorStatus, // TODO: add Terminated, Failed
    child_count: u32,
}

/// SSE GET: /api/v1/actors?groups=<addrs>

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActorUpdated {
    group: Addr,
    addr: Addr,
    key: String,
    status: elfo::ActorStatus,
}

/// SSE GET: /api/v1/metrics?groups=<addrs>,actors=<addrs>

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetricUpdated {
    // Meta.
    publisher: Addr,
    name: String, // "cpu_load", "mailbox_peak_100ms"
    labels: Vec<String>,

    // Dynamic values.
    published_at: Timestamp,
    value: MetricValue,
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum MetricValue {
    Gauge {
        last: f64,
    },
    Counter {
        count: u64,
        rate: f64,
    },
    Distribution {
        count: u64,
        mean: f64,
        min: f64,
        max: f64,
        p80: f64,
        p90: f64,
        p95: f64,
        p99: f64,
    },
}

impl Update {
    pub(crate) fn key(&self) -> UpdateKey {
        match self {
            Self::Heartbeat => UpdateKey::Heartbeat,
            Self::TopologyUpdated { .. } => UpdateKey::TopologyUpdated,
        }
    }
}

impl From<Topology> for Update {
    fn from(topology: Topology) -> Self {
        let groups = topology.actor_groups().map(Into::into).collect();
        let connections = topology.connections().map(Into::into).collect();
        Self::TopologyUpdated {
            connections,
            groups,
        }
    }
}
