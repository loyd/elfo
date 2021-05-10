use elfo::Context;
use elfo_core as elfo;
use futures::{Stream, StreamExt};
use tokio::{runtime::Handle, sync::mpsc::Receiver};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, warn};
use warp::{
    sse::{self},
    Filter,
};

use crate::{
    api::UpdateResult,
    protocol::{Config, Request, RequestBody, Token},
    values::UpdateError,
};

#[derive(Clone)]
pub(crate) struct InspectorServer {
    config: Config,
    ctx: Context,
}

impl InspectorServer {
    pub(crate) fn new(config: &Config, ctx: Context) -> Self {
        Self {
            config: config.clone(),
            ctx,
        }
    }

    pub(crate) async fn exec(self) {
        let ctx = self.ctx.clone();
        let auth_token = warp::header::<Token>("auth-token");
        let routes = warp::path!("api" / "v1" / "topology")
            .and(warp::get())
            .and(auth_token)
            .map(move |auth_token| {
                warp::sse::reply(request(ctx.clone(), auth_token, RequestBody::GetTopology))
            });
        warp::serve(routes)
            .run((self.config.ip, self.config.port))
            .await;
    }
}

fn request(
    ctx: Context,
    auth_token: Token,
    body: RequestBody,
) -> impl Stream<Item = Result<sse::Event, UpdateError>> {
    let (req, rx): (Request, Receiver<UpdateResult>) = Request::new(auth_token, body);
    let tx = req.tx().clone();
    if ctx.try_send_to(ctx.addr(), req).is_err() {
        if let Err(err) = Handle::current().block_on(tx.send(Err(UpdateError::TooManyRequests))) {
            error!(?err, "can't send error update");
        }
    }

    ReceiverStream::new(rx).map(move |update_result| -> Result<sse::Event, UpdateError> {
        update_result
            .and_then(|update| {
                sse::Event::default()
                    .event("update")
                    .json_data(update)
                    .map_err(UpdateError::from)
            })
            .map_err(|err| {
                warn!(?err, "can't handle connection");
                err
            })
    })
}
