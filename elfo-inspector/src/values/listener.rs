use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use slotmap::{new_key_type, SlotMap};
use tokio::sync::mpsc::Sender;

use elfo::time::Interval;
use elfo_core as elfo;

use crate::{
    api::UpdateResult,
    protocol::{HeartbeatTick, Request},
    values::{Uid, UpdateError},
};

/// A listener of _server-sent events_ (SSE).
/// Holds the connection to clients open only while they're ready to receive new
/// events. Sends _heartbeat_ messages to every client with specified period.
///
/// Heartbeats solve several problems:
/// - They allow client to be notified about stuck runtime node.
/// - Unsuccessful attempts to send the heartbeat notify us about client
///   disconnection.
pub(crate) struct Listener {
    pub(crate) heartbeat_at: Instant,
    pub(crate) heartbeat_uid: Uid,
    pub(crate) tx: Sender<UpdateResult>,
}

new_key_type! {
    pub(crate) struct ListenerKey;
}

impl Listener {
    pub(crate) fn new(
        tx: Sender<UpdateResult>,
        heartbeat_period: Duration,
        heartbeat_uid: Uid,
    ) -> Self {
        Self {
            heartbeat_at: Instant::now() + heartbeat_period,
            heartbeat_uid,
            tx,
        }
    }

    pub(crate) fn postpone_heartbeat(&mut self, heartbeat_period: Duration) {
        self.heartbeat_at = Instant::now() + heartbeat_period;
    }
}
