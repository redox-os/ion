//! Contains the logic for enabling foreground management.

use nix::unistd::Pid;

// use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub enum BackgroundResult {
    Errored,
    Status(i32),
}

const REPLIED: u8 = 1;
const ERRORED: u8 = 2;

#[derive(Debug)]
/// An atomic structure that can safely be shared across threads, which serves to provide
/// communication between the shell and background threads. The `fg` command uses this
/// structure to notify a background thread that it needs to wait for and return
/// the exit status back to the `fg` function.
pub struct Signals {
    grab:   AtomicUsize, // AtomicU32,
    status: AtomicUsize, // AtomicU8,
    reply:  AtomicUsize, // AtomicU8,
}

impl Signals {
    pub fn was_grabbed(&self, pid: Pid) -> bool {
        self.grab.load(Ordering::Relaxed) == pid.as_raw() as usize
    }

    pub fn was_processed(&self) -> Option<BackgroundResult> {
        let reply = self.reply.load(Ordering::Acquire) as u8;
        self.reply.store(0, Ordering::Relaxed);
        if reply == ERRORED {
            Some(BackgroundResult::Errored)
        } else if reply == REPLIED {
            Some(BackgroundResult::Status(self.status.load(Ordering::Relaxed) as i32))
        } else {
            None
        }
    }

    pub fn errored(&self) {
        self.grab.store(0, Ordering::Relaxed);
        self.reply.store(ERRORED as usize, Ordering::Release);
    }

    pub fn reply_with(&self, status: i32) {
        self.grab.store(0, Ordering::Relaxed);
        self.status.store(status as usize, Ordering::Relaxed);
        self.reply.store(REPLIED as usize, Ordering::Release);
    }

    pub fn signal_to_grab(&self, pid: Pid) {
        self.grab.store(pid.as_raw() as usize, Ordering::Relaxed);
    }

    pub const fn new() -> Self {
        Self {
            grab:   AtomicUsize::new(0),
            status: AtomicUsize::new(0),
            reply:  AtomicUsize::new(0),
        }
    }
}
