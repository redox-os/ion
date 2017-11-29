use super::{IonError, Shell};
use std::fs::File;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::process::exit;
use sys;

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
/// Instructs whether or not to capture the standard output and error streams.
///
/// A type that is utilized by the `Fork` structure.
pub enum Capture {
    /// Don't capture any streams at all.
    None = 0,
    /// Capture just the standard output stream.
    Stdout = 1,
    /// Capture just the standard error stream.
    Stderr = 2,
    /// Capture both the standard output and error streams.
    Both = 3,
}

/// Utilized by the shell for performing forks and capturing streams.
///
/// Using this structure directly is equivalent to using `Shell`'s fork method.
pub struct Fork<'a> {
    shell:   &'a Shell,
    capture: Capture,
}

#[derive(Debug)]
/// The result returned by a fork, which optionally contains file handles to the standard
/// output and error streams, as well as the PID of the child. This structure is subject to change
/// in the future, once there's a better means of obtaining the exit status without having to
/// wait on the PID.
pub struct IonResult {
    pub pid:    u32,
    pub stdout: Option<File>,
    pub stderr: Option<File>,
}

impl<'a> Fork<'a> {
    /// Creates a new `Fork` state from an existing shell.
    pub fn new(shell: &'a Shell, capture: Capture) -> Fork<'a> { Fork { shell, capture } }

    /// Executes a closure within the child of the fork, and returning an `IonResult` in a
    /// non-blocking fashion.
    pub fn exec<F: FnMut(&mut Shell)>(&self, mut child_func: F) -> Result<IonResult, IonError> {
        sys::signals::block();

        // If we are to capture stdout, create a pipe for capturing outputs.
        let outs = if self.capture as u8 & Capture::Stdout as u8 != 0 {
            Some(sys::pipe2(sys::O_CLOEXEC)
                .map(|fds| unsafe { (File::from_raw_fd(fds.0), File::from_raw_fd(fds.1)) })
                .map_err(IonError::Fork)?)
        } else {
            None
        };

        // And if we are to capture stderr, create a pipe for that as well.
        let errs = if self.capture as u8 & Capture::Stderr as u8 != 0 {
            Some(sys::pipe2(sys::O_CLOEXEC)
                .map(|fds| unsafe { (File::from_raw_fd(fds.0), File::from_raw_fd(fds.1)) })
                .map_err(IonError::Fork)?)
        } else {
            None
        };

        match unsafe { sys::fork() } {
            Ok(0) => {
                // Allow the child to handle it's own signal handling.
                sys::signals::unblock();

                // Redirect standard output to a pipe, if needed.
                if let Some((read, write)) = outs {
                    let _ = sys::dup2(write.as_raw_fd(), sys::STDOUT_FILENO);
                    drop(write);
                    drop(read);
                }

                // Redirect standard error to a pipe, if needed.
                if let Some((read, write)) = errs {
                    let _ = sys::dup2(write.as_raw_fd(), sys::STDERR_FILENO);
                    drop(write);
                    drop(read);
                }

                // Execute the given closure within the child's shell, and stop the context's
                // background thread. NOTE: That last thing solves a memory leak.
                let mut shell: Shell = unsafe { (self.shell as *const Shell).read() };
                shell.context.take().map(|mut c| c.history.commit_history());
                child_func(&mut shell);

                // Reap the child, enabling the parent to get EOF from the read end of the pipe.
                exit(shell.previous_status);
            }
            Ok(pid) => Ok(IonResult {
                pid,
                stdout: outs.map(|(read, write)| {
                    drop(write);
                    read
                }),
                stderr: errs.map(|(read, write)| {
                    drop(write);
                    read
                }),
            }),
            Err(why) => Err(IonError::Fork(why)),
        }
    }
}
