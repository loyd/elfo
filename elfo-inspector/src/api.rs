//! Types to use for communication with clients.
//! All endpoints can return 400, 401, 429 and 500.

use serde::Serialize;
use smartstring::alias::String;

use elfo::topology::{ActorGroup, Connection};
use elfo_core as elfo;
use elfo_macros::message;

type Addr = u64;
type Timestamp = f64;

/// SSE GET: /api/v1/topology

#[message(part, elfo = elfo_core)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TopologyUpdated {
    pub groups: Vec<TopologyActorGroup>,
    pub connections: Vec<TopologyConnection>,
}

#[message(part, elfo = elfo_core)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TopologyActorGroup {
    addr: Addr,
    name: String,
}

#[message(part, elfo = elfo_core)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TopologyConnection {
    from: Addr,
    to: Addr,
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

impl From<ActorGroup> for TopologyActorGroup {
    fn from(val: ActorGroup) -> Self {
        Self {
            addr: val.addr.into_bits() as _,
            name: val.name.into(),
        }
    }
}

impl From<Connection> for TopologyConnection {
    fn from(val: Connection) -> Self {
        Self {
            from: val.from.into_bits() as _,
            to: val.to.into_bits() as _,
        }
    }
}
