//! This module contains all of the code that manages signal handling in the shell. Primarily, this will be used to
//! block signals in the shell at startup, and unblock signals for each of the forked children of the shell.

use sys;

#[cfg(all(unix, not(target_os = "redox")))]
pub use self::unix::*;

#[cfg(target_os = "redox")]
pub use self::redox::*;

#[cfg(all(unix, not(target_os = "redox")))]
mod unix {
    /// Blocks the SIGTSTP/SIGTTOU/SIGTTIN/SIGCHLD signals so that the shell never receives them.
    pub fn block() {
        unsafe {
            use libc::*;
            use std::mem;
            use std::ptr;
            let mut sigset = mem::uninitialized::<sigset_t>();
            sigemptyset(&mut sigset as *mut sigset_t);
            sigaddset(&mut sigset as *mut sigset_t, SIGTSTP);
            sigaddset(&mut sigset as *mut sigset_t, SIGTTOU);
            sigaddset(&mut sigset as *mut sigset_t, SIGTTIN);
            sigaddset(&mut sigset as *mut sigset_t, SIGCHLD);
            sigprocmask(SIG_BLOCK, &sigset as *const sigset_t, ptr::null_mut() as *mut sigset_t);
        }
    }

    /// Unblocks the SIGTSTP/SIGTTOU/SIGTTIN/SIGCHLD signals so children processes can be controlled by the shell.
    pub fn unblock() {
        unsafe {
            use libc::*;
            use std::mem;
            use std::ptr;
            let mut sigset = mem::uninitialized::<sigset_t>();
            sigemptyset(&mut sigset as *mut sigset_t);
            sigaddset(&mut sigset as *mut sigset_t, SIGTSTP);
            sigaddset(&mut sigset as *mut sigset_t, SIGTTOU);
            sigaddset(&mut sigset as *mut sigset_t, SIGTTIN);
            sigaddset(&mut sigset as *mut sigset_t, SIGCHLD);
            sigprocmask(SIG_UNBLOCK, &sigset as *const sigset_t, ptr::null_mut() as *mut sigset_t);
        }
    }
}

// TODO
#[cfg(target_os = "redox")]
mod redox {
    pub fn block() { }

    pub fn unblock() { }
}

/// Suspends a given process by it's process ID.
pub fn suspend(pid: u32) {
    let _ = sys::killpg(pid, sys::SIGSTOP);
}

/// Resumes a given process by it's process ID.
pub fn resume(pid: u32) {
    let _ = sys::killpg(pid, sys::SIGCONT);
}

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
    fn drop(&mut self) {
        unblock();
    }
}
