//! This module contains all of the code that manages signal handling in the shell. Primarily, this
//! will be used to block signals in the shell at startup, and unblock signals for each of the forked
//! children of the shell.

use std::sync::atomic::{ATOMIC_USIZE_INIT, AtomicUsize};

use sys;

pub use sys::signals::{block, unblock};

pub static PENDING: AtomicUsize = ATOMIC_USIZE_INIT;

/// Suspends a given process by it's process ID.
pub fn suspend(pid: u32) { let _ = sys::killpg(pid, sys::SIGSTOP); }

/// Resumes a given process by it's process ID.
pub fn resume(pid: u32) { let _ = sys::killpg(pid, sys::SIGCONT); }

/// The purpose of the signal handler is to ignore signals when it is active, and then continue
/// listening to signals once the handler is dropped.
pub struct SignalHandler;

impl SignalHandler {
    pub fn new() -> SignalHandler {
        block();
        SignalHandler
    }
}

impl Drop for SignalHandler {
    fn drop(&mut self) { unblock(); }
}
