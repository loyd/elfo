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

/// Notifies you once about the end of time interval via [messages][Message].
/// `Timer` is stopped by default.
/// The timer can always be rescheduled via [`reset(deadline)`][reset] call.
///
/// [Message]: ../struct.Message.html
/// [reset]: #method.reset
pub struct Timer<F> {
    message_factory: F,
    state: Mutex<State>,
}

struct State {
    is_waiting: bool,
    sleep: Pin<Box<Sleep>>,
}

enum DeadlineState {
    Waiting,
    Waited,
}

impl<F> Timer<F> {
    pub fn new(f: F) -> Self {
        Self {
            message_factory: f,
            state: Mutex::new(State {
                is_waiting: false,
                sleep: Box::pin(time::sleep_until(Instant::now())),
            }),
        }
    }

    pub fn reset(&self, deadline: Instant) {
        let mut state = self.state.lock();
        state.is_waiting = true;
        state.sleep.as_mut().reset(deadline);
    }

    pub fn sleep(&self, duration: Duration) {
        let mut state = self.state.lock();
        state.is_waiting = true;
        state.sleep.as_mut().reset(Instant::now() + duration);
    }
}

impl<M, F> Source for Timer<F>
where
    F: Fn() -> M,
    M: Message,
{
    fn poll_recv(&self, cx: &mut task::Context<'_>) -> Poll<Option<Envelope>> {
        let mut state = self.state.lock();

        if state.sleep.as_mut().poll(cx).is_ready() {
            // It hasn't been configured, so just ignore it.
            if state.is_waiting {
                state.is_waiting = false;

                // Emit a message.
                let message = (self.message_factory)();
                let kind = MessageKind::Regular { sender: Addr::NULL };
                let envelope = Envelope::new(message, kind).upcast();
                return Poll::Ready(Some(envelope));
            }

            // Now reset the underlying timer.
            // let period = state.period;
            // let sleep = state.sleep.as_mut();
            // let new_deadline = sleep.deadline() + period;
            // sleep.reset(new_deadline);
        }

        Poll::Pending // The timer can always be rescheduled
    }
}
