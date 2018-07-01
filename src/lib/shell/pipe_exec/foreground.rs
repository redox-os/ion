//! Contains the logic for enabling foreground management.

// use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub(crate) enum BackgroundResult {
    Errored,
    Status(u8),
}

const REPLIED: u8 = 1;
const ERRORED: u8 = 2;

#[derive(Debug)]
/// An atomic structure that can safely be shared across threads, which serves to provide
/// communication between the shell and background threads. The `fg` command uses this
/// structure to notify a background thread that it needs to wait for and return
/// the exit status back to the `fg` function.
pub(crate) struct ForegroundSignals {
    grab:   AtomicUsize, // AtomicU32,
    status: AtomicUsize, // AtomicU8,
    reply:  AtomicUsize, // AtomicU8,
}

impl ForegroundSignals {
    pub(crate) fn was_grabbed(&self, pid: u32) -> bool { self.grab.load(Ordering::SeqCst) as u32 == pid }

    pub(crate) fn was_processed(&self) -> Option<BackgroundResult> {
        let reply = self.reply.load(Ordering::SeqCst) as u8;
        self.reply.store(0, Ordering::SeqCst);
        if reply & ERRORED != 0 {
            Some(BackgroundResult::Errored)
        } else if reply & REPLIED != 0 {
            Some(BackgroundResult::Status(
                self.status.load(Ordering::SeqCst) as u8
            ))
        } else {
            None
        }
    }

    pub(crate) fn errored(&self) {
        self.grab.store(0, Ordering::SeqCst);
        self.reply.store(ERRORED as usize, Ordering::SeqCst);
    }

    pub(crate) fn reply_with(&self, status: i8) {
        self.grab.store(0, Ordering::SeqCst);
        self.status.store(status as usize, Ordering::SeqCst);
        self.reply.store(REPLIED as usize, Ordering::SeqCst);
    }

    pub(crate) fn signal_to_grab(&self, pid: u32) { self.grab.store(pid as usize, Ordering::SeqCst); }

    pub(crate) fn new() -> ForegroundSignals {
        ForegroundSignals {
            grab:   AtomicUsize::new(0),
            status: AtomicUsize::new(0),
            reply:  AtomicUsize::new(0),
        }
    }
}
