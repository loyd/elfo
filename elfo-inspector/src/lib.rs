#![warn(rust_2018_idioms, unreachable_pub)]

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use priority_queue::PriorityQueue;
use slotmap::{new_key_type, SlotMap};
use tokio::sync::mpsc::Sender;
use tracing::error;

use elfo::{time::Interval, ActorGroup, Context, Schema, Topology};
use elfo_core as elfo;
use elfo_macros::msg_raw as msg;

mod api;
mod channel;
mod protocol;
mod server;
mod values;

use api::{Update, UpdateKey, UpdateResult};
use channel::Channel;
use protocol::{Config, HeartbeatTick, Request, RequestBody, TopologyUpdated};
use server::InspectorServer;
use values::{InspectorError, Listener, ListenerKey, Uid, UidGenerator};

struct Inspector {
    channels: BTreeMap<UpdateKey, Channel>,
    heartbeat_indexes: UidGenerator,
    heartbeat_period: Duration,
    heartbeats: BTreeMap<Uid, ListenerKey>,
    listeners: SlotMap<ListenerKey, Listener>,
    topology: Topology,
}

pub fn new(topology: Topology) -> Schema {
    let topology = Arc::new(topology);
    ActorGroup::new().config::<Config>().exec(move |ctx| {
        let topology = topology.as_ref().clone();
        let inspector = Inspector::new(&ctx, topology);
        inspector.exec(ctx)
    })
}

impl Inspector {
    fn new(ctx: &Context<Config>, topology: Topology) -> Self {
        Self {
            channels: BTreeMap::<UpdateKey, Channel>::new(),
            heartbeat_indexes: Default::default(),
            heartbeat_period: ctx.config().heartbeat_period,
            heartbeats: BTreeMap::<Uid, ListenerKey>::new(),
            listeners: Default::default(),
            topology,
        }
    }

    async fn exec(mut self, ctx: Context<Config>) {
        let server = InspectorServer::new(ctx.config(), ctx.pruned());
        let mut server_execution = tokio::spawn(server.exec());
        let &Config {
            heartbeat_period, ..
        } = ctx.config();
        let heartbeat_interval = Interval::new(|| HeartbeatTick);
        let mut ctx = ctx.with(&heartbeat_interval);
        loop {
            tokio::select! {
                Some(envelope) = ctx.recv() => {
                    msg!(match envelope {
                        req @ Request => {
                            match req.body {
                                RequestBody::GetTopology => {
                                    let update: Update = self.topology.clone().into();
                                    let update_key = update.key();
                                    match self.subscribe(update_key, req.tx().clone(), &heartbeat_interval) {
                                        Ok(listener_key) => {
                                            if let Err(err) = self.send(listener_key, update) {
                                                error!(?err, "can't send the snapshot to the listener");
                                            }
                                        },
                                        Err(err) => {
                                            error!(?err, "can't subscribe the listener");
                                        },
                                    }
                                },
                            }
                        },
                        HeartbeatTick => {
                            if let Err(err) = self.heartbeat_tick(&heartbeat_interval) {
                                error!("can't handle heartbeat tick");
                            }
                        },
                        TopologyUpdated => {
                            self.send_to_channel(self.topology.clone().into());
                        },
                        _ => {},
                    });
                },
                _ = &mut server_execution => {
                    break;
                },
            };
        }
    }

    fn send_to_channel(&mut self, update: Update) -> Result<()> {
        let update_key = update.key();
        let mut channel = if let Some(channel) = self.channels.get_mut(&update_key) {
            channel
        } else {
            return Err(InspectorError::new("there're no listeners for such updates").into());
        };
        if let Err(err) = channel.send(&update, &mut self.listeners) {
            self.channels.remove(&update_key);
            return Err(err);
        }
        Ok(())
    }

    fn subscribe<F: Fn() -> HeartbeatTick>(
        &mut self,
        update_key: UpdateKey,
        tx: Sender<UpdateResult>,
        interval: &Interval<F>,
    ) -> Result<ListenerKey> {
        let mut channel = if let Some(channel) = self.channels.get_mut(&update_key) {
            channel
        } else {
            let channel = Channel::new(self.heartbeat_period);
            self.channels.insert(update_key, channel);
            self.channels.get_mut(&update_key).unwrap()
        };
        let listener = Listener::new(tx, self.heartbeat_period, self.heartbeat_indexes.next());
        let listener_key = self.listeners.insert(listener);
        channel.subscribe(listener_key);
        if self.listeners.is_empty() {
            interval.set_period(self.heartbeat_period);
        }
        self.schedule_heartbeat(listener_key)?;
        Ok(listener_key)
    }

    fn send(&mut self, listener_key: ListenerKey, update: Update) -> Result<()> {
        let listener = if let Some(listener) = self.listeners.get(listener_key) {
            listener
        } else {
            return Err(InspectorError::new("there's no such listener").into());
        };
        if listener.tx.try_send(Ok(update)).is_err() {
            self.listeners.remove(listener_key);
            return Err(InspectorError::new("listener doesn't receive").into());
        }
        self.postpone_heartbeat(listener_key)
    }

    fn heartbeat_tick<F: Fn() -> HeartbeatTick>(&mut self) -> Result<()> {
        let now = Instant::now();
        while let Some((&uid, &listener_key)) = self.heartbeats.iter().next() {
            let listener = if let Some(listener) = self.listeners.get(listener_key) {
                listener
            } else {
                self.heartbeats.remove(&uid);
                continue;
            };
            if listener.heartbeat_at > now {
                break;
            }
            self.heartbeats.remove(&uid);
            if let Err(err) = self.send(listener_key, Update::Heartbeat) {
                error!(?err, "can't send heartbeat")
            }
        }
        Ok(())
    }

    fn postpone_heartbeat(&mut self, listener_key: ListenerKey) -> Result<()> {
        let listener = if let Some(listener) = self.listeners.get_mut(listener_key) {
            listener
        } else {
            return Err(InspectorError::new("there's no such listener").into());
        };
        self.heartbeats.remove(&listener.heartbeat_uid);
        listener.heartbeat_at = Instant::now() + self.heartbeat_period;
        listener.heartbeat_uid = self.heartbeat_indexes.next();
        self.schedule_heartbeat(listener_key)
    }

    fn schedule_heartbeat(&mut self, listener_key: ListenerKey) -> Result<()> {
        let listener = if let Some(listener) = self.listeners.get(listener_key) {
            listener
        } else {
            return Err(InspectorError::new("there's no such listener").into());
        };
        self.heartbeats.insert(listener.heartbeat_uid, listener_key);
        Ok(())
    }
}
