//! Contains the logic for enabling foreground management.

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

pub(crate) enum BackgroundResult {
    Errored,
    Status(u8),
}

const REPLIED: u8 = 1;
const ERRORED: u8 = 2;

/// An atomic structure that can safely be shared across threads, which serves to provide
/// communication between the shell and background threads. The `fg` command uses this
/// structure to notify a background thread that it needs to wait for and return
/// the exit status back to the `fg` function.
pub(crate) struct ForegroundSignals {
    grab:   AtomicU32,
    status: AtomicU8,
    reply:  AtomicU8,
}

impl ForegroundSignals {
    pub(crate) fn was_grabbed(&self, pid: u32) -> bool { self.grab.load(Ordering::Relaxed) == pid }

    pub(crate) fn was_processed(&self) -> Option<BackgroundResult> {
        let reply = self.reply.load(Ordering::Relaxed);
        self.reply.store(0, Ordering::Relaxed);
        if reply & ERRORED != 0 {
            Some(BackgroundResult::Errored)
        } else if reply & REPLIED != 0 {
            Some(BackgroundResult::Status(
                self.status.load(Ordering::Relaxed) as u8,
            ))
        } else {
            None
        }
    }

    pub(crate) fn errored(&self) {
        self.grab.store(0, Ordering::Relaxed);
        self.reply.store(ERRORED, Ordering::Relaxed);
    }

    pub(crate) fn reply_with(&self, status: i8) {
        self.grab.store(0, Ordering::Relaxed);
        self.status.store(status as u8, Ordering::Relaxed);
        self.reply.store(REPLIED, Ordering::Relaxed);
    }

    pub(crate) fn signal_to_grab(&self, pid: u32) { self.grab.store(pid, Ordering::Relaxed); }

    pub(crate) fn new() -> ForegroundSignals {
        ForegroundSignals {
            grab:   AtomicU32::new(0),
            status: AtomicU8::new(0),
            reply:  AtomicU8::new(0),
        }
    }
}
