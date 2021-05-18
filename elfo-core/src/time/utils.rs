use std::{
    future::Future,
    pin::Pin,
    task::{self, Poll},
};

use parking_lot::Mutex;
use tokio::time::{self, Duration, Instant, Sleep};

use crate::{
    addr::Addr,
    context::Source,
    envelope::{Envelope, MessageKind},
    message::Message,
};

pub(crate) struct State {
    pub(crate) sleep: Pin<Box<Sleep>>,
    pub(crate) start_at: Option<Instant>,
    pub(crate) period: Duration,
}
