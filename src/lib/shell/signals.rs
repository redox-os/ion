//! This module contains all of the code that manages signal handling in the
//! shell. Primarily, this will be used to block signals in the shell at
//! startup, and unblock signals for each of the forked
//! children of the shell.

// use std::sync::atomic::{ATOMIC_U8_INIT, AtomicU8};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::sys;
pub use super::sys::signals::{block, unblock};

pub static PENDING: AtomicUsize = AtomicUsize::new(0);
pub const SIGINT: u8 = 1;
pub const SIGHUP: u8 = 2;
pub const SIGTERM: u8 = 4;

/// Resumes a given process by it's process ID.
pub fn resume(pid: u32) { let _ = sys::killpg(pid, libc::SIGCONT); }

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

impl Iterator for SignalHandler {
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        match PENDING.swap(0, Ordering::SeqCst) as u8 {
            0 => None,
            SIGINT => Some(libc::SIGINT),
            SIGHUP => Some(libc::SIGHUP),
            SIGTERM => Some(libc::SIGTERM),
            _ => unreachable!(),
        }
    }
}
