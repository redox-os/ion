//! This module contains all of the code that manages signal handling in the
//! shell. Primarily, this will be used to block signals in the shell at
//! startup, and unblock signals for each of the forked
//! children of the shell.

// use std::sync::atomic::{ATOMIC_U8_INIT, AtomicU8};
use std::sync::atomic::{AtomicUsize, Ordering};

pub use super::sys::signals::{block, unblock};
use nix::{sys::signal, unistd::Pid};

pub static PENDING: AtomicUsize = AtomicUsize::new(0);
pub const SIGINT: u8 = 1;
pub const SIGHUP: u8 = 2;
pub const SIGTERM: u8 = 4;

/// Resumes a given process by it's process ID.
pub fn resume(pid: Pid) { let _ = signal::killpg(pid, signal::Signal::SIGCONT); }

/// The purpose of the signal handler is to ignore signals when it is active, and then continue
/// listening to signals once the handler is dropped.
pub struct SignalHandler;

impl SignalHandler {
    pub fn new() -> Self {
        block();
        SignalHandler
    }
}

impl Drop for SignalHandler {
    fn drop(&mut self) { unblock(); }
}

impl Iterator for SignalHandler {
    type Item = signal::Signal;

    fn next(&mut self) -> Option<Self::Item> {
        match PENDING.swap(0, Ordering::SeqCst) as u8 {
            0 => None,
            SIGINT => Some(signal::Signal::SIGINT),
            SIGHUP => Some(signal::Signal::SIGHUP),
            SIGTERM => Some(signal::Signal::SIGTERM),
            _ => unreachable!(),
        }
    }
}
