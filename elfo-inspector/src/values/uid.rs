use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) struct UidGenerator {
    next: AtomicU64,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Uid(u64);

impl UidGenerator {
    pub(crate) fn next(&self) -> Uid {
        let next = self.next.fetch_add(1, Ordering::SeqCst);
        Uid(next)
    }
}

impl Default for UidGenerator {
    fn default() -> Self {
        Self { next: 0.into() }
    }
}
