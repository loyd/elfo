use elfo::{Context, Topology};
use elfo_core as elfo;
use futures::{Stream, StreamExt};
use tokio::sync::mpsc::Receiver;
use tokio_stream::wrappers::ReceiverStream;
use warp::{
    sse::{self},
    Filter,
};

use crate::{
    protocol::{Config, GetTopology, Request, Token},
    values::UpdateError,
};

#[derive(Clone)]
pub(crate) struct InspectorServer {
    config: Config,
    ctx: Context,
    topology: Topology,
}

impl InspectorServer {
    pub(crate) fn new(config: &Config, ctx: Context, topology: Topology) -> Self {
        Self {
            config: config.clone(),
            ctx,
            topology,
        }
    }

    pub(crate) async fn exec(self) {
        let ctx = self.ctx.clone();
        let auth_token = warp::header::<Token>("auth-token");
        let routes = warp::path!("api" / "v1" / "topology")
            .and(warp::get())
            .and(auth_token)
            .map(move |auth_token| {
                warp::sse::reply(request(ctx.clone(), GetTopology::new(auth_token)))
            });
        warp::serve(routes)
            .run((self.config.ip, self.config.port))
            .await;
    }
}

fn request<R: Request>(
    ctx: Context,
    (req, rx): (R, Receiver<Result<R::Update, UpdateError>>),
) -> impl Stream<Item = Result<sse::Event, UpdateError>> {
    let tx = req.tx().clone();
    if ctx.try_send_to(ctx.addr(), req).is_err() {
        tx.send(Err(UpdateError::TooManyRequests));
    }

    let event_stream =
        ReceiverStream::new(rx).map(move |update_result| -> Result<sse::Event, UpdateError> {
            update_result.and_then(|update| {
                sse::Event::default()
                    .event("update")
                    .json_data(update)
                    .map_err(UpdateError::from)
            })
        });
    Box::new(event_stream)
}
