use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use slotmap::SlotMap;
use tokio::sync::mpsc::Sender;

use elfo::time::Interval;
use elfo_core as elfo;

use crate::{
    api::{Update, UpdateResult},
    protocol::{HeartbeatTick, Request},
    values::{InspectorError, Listener, ListenerKey, UpdateError},
};

/// A stream of _server-sent events_ (SSE).
/// Holds the connection to clients open only while they're ready to receive new
/// events. Sends _heartbeat_ messages to every client with specified period.
///
/// Heartbeats solve several problems:
/// - They allow client to be notified about stuck runtime node.
/// - Unsuccessful attempts to send the heartbeat notify us about client
///   disconnection.
pub(crate) struct Channel {
    heartbeat_period: Duration,
    listeners_keys: BTreeSet<ListenerKey>,
}

impl Channel {
    pub(crate) fn new(heartbeat_period: Duration) -> Self {
        Self {
            heartbeat_period,
            listeners_keys: Default::default(),
        }
    }

    pub(crate) fn send(
        &mut self,
        update: &Update,
        listeners: &mut SlotMap<ListenerKey, Listener>,
    ) -> Result<()> {
        let mut is_dry_run = true;
        let keys: Vec<ListenerKey> = self.listeners_keys.iter().cloned().collect();
        for key in keys.into_iter() {
            let mut listener = if let Some(listener) = listeners.get_mut(key) {
                listener
            } else {
                self.listeners_keys.remove(&key);
                continue;
            };
            if let Err(err) = listener.tx.try_send(Ok(update.clone())) {
                self.listeners_keys.remove(&key);
                listeners.remove(key);
                continue;
            }
            listener.postpone_heartbeat(self.heartbeat_period);
            is_dry_run = false;
        }
        if is_dry_run {
            Err(InspectorError::new("there're no listeners for such updates").into())
        } else {
            Ok(())
        }
    }

    pub(crate) fn subscribe(&mut self, key: ListenerKey) {
        self.listeners_keys.insert(key);
    }
}
