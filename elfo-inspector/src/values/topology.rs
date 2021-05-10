use smartstring::alias::String;
use tokio::sync::mpsc::Sender;

use elfo::topology::{ActorGroup, Connection};
use elfo_core as elfo;
use elfo_macros::message;

use crate::values::Addr;

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
