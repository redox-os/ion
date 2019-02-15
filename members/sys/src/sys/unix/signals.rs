use libc::*;
use std::{mem, ptr};

/// Blocks the SIGTSTP/SIGTTOU/SIGTTIN/SIGCHLD signals so that the shell never receives
/// them.
pub fn block() {
    unsafe {
        let mut sigset = mem::uninitialized::<sigset_t>();
        sigemptyset(&mut sigset as *mut sigset_t);
        sigaddset(&mut sigset as *mut sigset_t, SIGTSTP);
        sigaddset(&mut sigset as *mut sigset_t, SIGTTOU);
        sigaddset(&mut sigset as *mut sigset_t, SIGTTIN);
        sigaddset(&mut sigset as *mut sigset_t, SIGCHLD);
        sigprocmask(SIG_BLOCK, &sigset as *const sigset_t, ptr::null_mut() as *mut sigset_t);
    }
}

/// Unblocks the SIGTSTP/SIGTTOU/SIGTTIN/SIGCHLD signals so children processes can be
/// controlled
/// by the shell.
pub fn unblock() {
    unsafe {
        let mut sigset = mem::uninitialized::<sigset_t>();
        sigemptyset(&mut sigset as *mut sigset_t);
        sigaddset(&mut sigset as *mut sigset_t, SIGTSTP);
        sigaddset(&mut sigset as *mut sigset_t, SIGTTOU);
        sigaddset(&mut sigset as *mut sigset_t, SIGTTIN);
        sigaddset(&mut sigset as *mut sigset_t, SIGCHLD);
        sigprocmask(SIG_UNBLOCK, &sigset as *const sigset_t, ptr::null_mut() as *mut sigset_t);
    }
}
