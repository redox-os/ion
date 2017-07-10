//! This module contains all of the code that manages signal handling in the shell. Primarily, this will be used to
//! block signals in the shell at startup, and unblock signals for each of the forked children of the shell.

#[cfg(all(unix, not(target_os = "redox")))]
pub use self::unix::*;

#[cfg(target_os = "redox")]
pub use self::redox::*;

#[cfg(all(unix, not(target_os = "redox")))]
mod unix {
    use futures::{Future, Stream};
    use nix::sys::signal::{kill, Signal};
    use std::sync::mpsc::Sender;
    use tokio_core::reactor::Core;
    use tokio_signal::unix::{self as unix_signal, Signal as TokioSignal};

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

    /// Suspends a given process by it's process ID.
    pub fn suspend(pid: u32) {
        let _ = kill(-(pid as i32), Some(Signal::SIGSTOP));
    }

    /// Resumes a given process by it's process ID.
    pub fn resume(pid: u32) {
        let _ = kill(-(pid as i32), Some(Signal::SIGCONT));
    }


    /// Execute the event loop that will listen for and transmit received signals to the shell.
    pub fn event_loop(signals_tx: Sender<i32>) {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        // Block the SIGTSTP signal -- prevents the shell from being stopped
        // when the foreground group is changed during command execution.
        block();

        // Create a stream that will select over SIGINT, SIGTERM, and SIGHUP signals.
        let signals = TokioSignal::new(unix_signal::SIGINT, &handle).flatten_stream()
            .select(TokioSignal::new(unix_signal::SIGTERM, &handle).flatten_stream())
            .select(TokioSignal::new(unix_signal::SIGHUP, &handle).flatten_stream());

        core.run(signals.for_each(|signal| {
            let _ = signals_tx.send(signal);
            Ok(())
        })).unwrap();
    }
}

// TODO
#[cfg(target_os = "redox")]
mod redox {
    use syscall;

    pub fn block() { }

    pub fn unblock() { }

    pub fn suspend(pid: u32) {
        let _ = syscall::kill(pid as usize, syscall::SIGSTOP);
    }

    pub fn resume(pid: u32) {
        let _ = syscall::kill(pid as usize, syscall::SIGCONT);
    }
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
