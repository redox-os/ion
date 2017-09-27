//! Contains the logic for enabling foreground management.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub(crate) enum BackgroundResult {
    Errored,
    Status(u8),
}

/// An atomic structure that can safely be shared across threads, which serves to provide
/// communication between the shell and background threads. The `fg` command uses this
/// structure to notify a background thread that it needs to wait for and return
/// the exit status back to the `fg` function.
pub(crate) struct ForegroundSignals {
    grab:    AtomicUsize, // TODO: Use AtomicU32 when stable
    status:  AtomicUsize, // TODO: Use AtomicU8 when stable
    reply:   AtomicBool,
    errored: AtomicBool, // TODO: Combine with reply when U8 is stable
}

impl ForegroundSignals {
    pub(crate) fn new() -> ForegroundSignals {
        ForegroundSignals {
            grab:    AtomicUsize::new(0),
            status:  AtomicUsize::new(0),
            reply:   AtomicBool::new(false),
            errored: AtomicBool::new(false),
        }
    }

    pub(crate) fn signal_to_grab(&self, pid: u32) {
        self.grab.store(pid as usize, Ordering::Relaxed);
    }

    pub(crate) fn reply_with(&self, status: i8) {
        self.grab.store(0, Ordering::Relaxed);
        self.status.store(status as usize, Ordering::Relaxed);
        self.reply.store(true, Ordering::Relaxed);
    }

    pub(crate) fn errored(&self) {
        self.grab.store(0, Ordering::Relaxed);
        self.errored.store(true, Ordering::Relaxed);
        self.reply.store(true, Ordering::Relaxed);
    }

    pub(crate) fn was_processed(&self) -> Option<BackgroundResult> {
        if self.reply.load(Ordering::Relaxed) {
            self.reply.store(false, Ordering::Relaxed);
            if self.errored.load(Ordering::Relaxed) {
                self.errored.store(false, Ordering::Relaxed);
                Some(BackgroundResult::Errored)
            } else {
                Some(BackgroundResult::Status(self.status.load(Ordering::Relaxed) as u8))
            }
        } else {
            None
        }
    }

    pub(crate) fn was_grabbed(&self, pid: u32) -> bool {
        self.grab.load(Ordering::Relaxed) == pid as usize
    }
}
