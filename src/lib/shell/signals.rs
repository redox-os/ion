//! This module contains all of the code that manages signal handling in the
//! shell. Primarily, this will be used to block signals in the shell at
//! startup, and unblock signals for each of the forked
//! children of the shell.

// use std::sync::atomic::{ATOMIC_U8_INIT, AtomicU8};
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// Blocks the SIGTSTP/SIGTTOU/SIGTTIN/SIGCHLD signals so that the shell never receives
/// them.
pub fn block() {
    let mut sigset = signal::SigSet::empty();
    sigset.add(signal::Signal::SIGTSTP);
    sigset.add(signal::Signal::SIGTTOU);
    sigset.add(signal::Signal::SIGTTIN);
    sigset.add(signal::Signal::SIGCHLD);
    signal::sigprocmask(signal::SigmaskHow::SIG_BLOCK, Some(&sigset), None)
        .expect("Could not block the signals");
}

/// Unblocks the SIGTSTP/SIGTTOU/SIGTTIN/SIGCHLD signals so children processes can be
/// controlled
/// by the shell.
pub fn unblock() {
    let mut sigset = signal::SigSet::empty();
    sigset.add(signal::Signal::SIGTSTP);
    sigset.add(signal::Signal::SIGTTOU);
    sigset.add(signal::Signal::SIGTTIN);
    sigset.add(signal::Signal::SIGCHLD);
    signal::sigprocmask(signal::SigmaskHow::SIG_UNBLOCK, Some(&sigset), None)
        .expect("Could not block the signals");
}
