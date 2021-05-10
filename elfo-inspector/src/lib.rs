use std::sync::Arc;

use tracing::error;

use elfo::{ActorGroup, Context, Schema, Topology};
use elfo_core as elfo;
use elfo_macros::msg_raw as msg;

mod api;
mod protocol;
mod server;
mod values;

use api::TopologyUpdated;
use protocol::{Config, GetTopology, Request};
use server::InspectorServer;

struct Inspector {
    ctx: Context<Config>,
    server: InspectorServer,
    topology: Topology,
}

pub fn new(topology: Topology) -> Schema {
    let topology = Arc::new(topology);
    ActorGroup::new().config::<Config>().exec(move |ctx| {
        let topology = topology.as_ref().clone();
        let inspector = Inspector::new(ctx, topology);
        inspector.exec()
    })
}

impl Inspector {
    fn new(ctx: Context<Config>, topology: Topology) -> Self {
        let server = InspectorServer::new(ctx.config(), ctx.pruned());
        Self {
            ctx,
            server,
            topology,
        }
    }

    async fn exec(mut self) {
        let mut server_execution = tokio::spawn(self.server.exec());
        loop {
            tokio::select! {
                Some(envelope) = self.ctx.recv() => {
                    msg!(match envelope {
                        req @ GetTopology => {
                            let groups = self.topology.actor_groups().map(Into::into).collect();
                            let connections = self.topology.connections().map(Into::into).collect();
                            if let Err(err) = req.tx().try_send(Ok(TopologyUpdated {
                                groups,
                                connections,
                            })) {
                                error!(?err, "err");
                            }
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
}
