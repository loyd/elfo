use std::fmt;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{
    addr::Addr,
    envelope::Envelope,
    errors::{SendError, TryRecvError, TrySendError},
    mailbox::Mailbox,
    request_table::RequestTable,
};

pub(crate) struct Actor {
    mailbox: Mailbox,
    request_table: RequestTable,
    control: RwLock<ControlBlock>,
}

struct ControlBlock {
    status: ActorStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActorStatus {
    kind: ActorStatusKind,
    details: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum ActorStatusKind {
    Normal,
    Initializing,
    Alarming,
    Failed,
    Terminated,
}

impl fmt::Display for ActorStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.details {
            Some(details) => write!(f, "{:?}: {}", self.kind, details),
            None => write!(f, "{:?}", self.kind),
        }
    }
}

impl ActorStatus {
    pub const ALARMING: ActorStatus = ActorStatus::new(ActorStatusKind::Alarming);
    pub(crate) const FAILED: ActorStatus = ActorStatus::new(ActorStatusKind::Failed);
    pub const INITIALIZING: ActorStatus = ActorStatus::new(ActorStatusKind::Initializing);
    pub const NORMAL: ActorStatus = ActorStatus::new(ActorStatusKind::Normal);
    pub(crate) const TERMINATED: ActorStatus = ActorStatus::new(ActorStatusKind::Terminated);

    const fn new(kind: ActorStatusKind) -> Self {
        Self {
            kind,
            details: None,
        }
    }

    pub fn with_details(&self, details: impl fmt::Display) -> Self {
        ActorStatus {
            kind: self.kind.clone(),
            details: Some(format!("{}", details)),
        }
    }

    pub(crate) fn is_failed(&self) -> bool {
        self.kind == ActorStatusKind::Failed
    }
}

impl Actor {
    pub(crate) fn new(addr: Addr) -> Self {
        Actor {
            mailbox: Mailbox::new(),
            request_table: RequestTable::new(addr),
            control: RwLock::new(ControlBlock {
                status: ActorStatus::INITIALIZING,
            }),
        }
    }

    pub(crate) fn try_send(&self, envelope: Envelope) -> Result<(), TrySendError<Envelope>> {
        if self.is_closed() {
            return Err(TrySendError::Closed(envelope));
        }
        self.mailbox.try_send(envelope)
    }

    pub(crate) async fn send(&self, envelope: Envelope) -> Result<(), SendError<Envelope>> {
        if self.is_closed() {
            return Err(SendError(envelope));
        }
        self.mailbox.send(envelope).await
    }

    pub(crate) async fn recv(&self) -> Option<Envelope> {
        if self.is_initializing() {
            self.set_status(ActorStatus::NORMAL);
        }

        self.mailbox.recv().await
    }

    pub(crate) fn try_recv(&self) -> Result<Envelope, TryRecvError> {
        if self.is_initializing() {
            self.set_status(ActorStatus::NORMAL);
        }

        self.mailbox.try_recv()
    }

    pub(crate) fn request_table(&self) -> &RequestTable {
        &self.request_table
    }

    pub(crate) fn set_status(&self, status: ActorStatus) {
        let mut control = self.control.write();

        let is_good_kind = matches!(
            status.kind,
            ActorStatusKind::Normal | ActorStatusKind::Initializing | ActorStatusKind::Terminated
        );

        if let Some(details) = status.details.as_deref() {
            if is_good_kind {
                info!(status = ?status.kind, %details, "status changed");
            } else {
                error!(status = ?status.kind, %details, "status changed");
            }
        } else if is_good_kind {
            info!(status = ?status.kind, "status changed");
        } else {
            error!(status = ?status.kind, "status changed");
        };

        control.status = status;

        // TODO: use `sdnotify` to provide a detailed status to systemd.
    }

    pub(crate) fn is_closed(&self) -> bool {
        matches!(
            self.control.read().status.kind,
            ActorStatusKind::Failed | ActorStatusKind::Terminated
        )
    }

    pub(crate) fn is_initializing(&self) -> bool {
        matches!(
            self.control.read().status.kind,
            ActorStatusKind::Initializing
        )
    }
}
