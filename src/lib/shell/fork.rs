use super::{sys, IonError, Shell};
use nix::{
    fcntl::OFlag,
    sys::wait::{self, WaitPidFlag, WaitStatus},
    unistd::{self, ForkResult, Pid},
};
use std::{
    fs::File,
    os::unix::io::{AsRawFd, FromRawFd},
};

pub fn wait_for_child(pid: unistd::Pid) -> nix::Result<i32> {
    loop {
        if let WaitStatus::Exited(_, status) = wait::waitpid(pid, Some(WaitPidFlag::WUNTRACED))? {
            break Ok(status);
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
/// Instructs whether or not to capture the standard output and error streams.
///
/// A type that is utilized by the `Fork` structure.
pub enum Capture {
    /// Don't capture any streams at all.
    None = 0b0000,
    /// Capture just the standard output stream.
    Stdout = 0b0001,
    /// Capture just the standard error stream.
    Stderr = 0b0010,
    /// Capture both the standard output and error streams.
    Both = 0b0011,
    /// Redirect just the stdandard output stream to /dev/null.
    IgnoreStdout = 0b0100,
    /// Redirect just the standard error stream to /dev/null.
    IgnoreStderr = 0b1000,
    /// Redirect both the standard output and error streams to /dev/null.
    IgnoreBoth = 0b1100,
    /// Capture standard output and ignore standard error.
    StdoutThenIgnoreStderr = 0b1001,
    /// Capture standard error and ignore standard output.
    StderrThenIgnoreStdout = 0b0110,
}

/// Utilized by the shell for performing forks and capturing streams.
///
/// Using this structure directly is equivalent to using `Shell`'s fork method.
pub struct Fork<'a, 'b: 'a> {
    shell:   &'a Shell<'b>,
    capture: Capture,
}

#[derive(Debug)]
/// The result returned by a fork, which optionally contains file handles to the standard
/// output and error streams, as well as the PID of the child. This structure is subject to change
/// in the future, once there's a better means of obtaining the exit status without having to
/// wait on the PID.
pub struct IonResult {
    pub child:  Pid,
    pub stdout: Option<File>,
    pub stderr: Option<File>,
    pub status: i32,
}

impl<'a, 'b> Fork<'a, 'b> {
    /// Executes a closure within the child of the fork, and returning an `IonResult` in a
    /// non-blocking fashion.
    pub fn exec<F: FnMut(&mut Shell<'b>) -> Result<(), IonError> + 'a>(
        self,
        mut child_func: F,
    ) -> nix::Result<IonResult> {
        sys::signals::block();

        // If we are to capture stdout, create a pipe for capturing outputs.
        let outs = if self.capture as u8 & Capture::Stdout as u8 != 0 {
            let fds = unistd::pipe2(OFlag::O_CLOEXEC)?;
            Some(unsafe { (File::from_raw_fd(fds.0), File::from_raw_fd(fds.1)) })
        } else {
            None
        };

        // And if we are to capture stderr, create a pipe for that as well.
        let errs = if self.capture as u8 & Capture::Stderr as u8 != 0 {
            let fds = unistd::pipe2(OFlag::O_CLOEXEC)?;
            Some(unsafe { (File::from_raw_fd(fds.0), File::from_raw_fd(fds.1)) })
        } else {
            None
        };

        // TODO: Have a global static store a File that points to /dev/null (or :null for Redox)
        // at the beginning of the program so that any request for /dev/null's fd doesn't need to
        // be repeated.
        let null_file = File::open(sys::NULL_PATH);

        match unistd::fork()? {
            ForkResult::Child => {
                // Allow the child to handle it's own signal handling.
                sys::signals::unblock();

                // Redirect standard output to a pipe, or /dev/null, if needed.
                if self.capture as u8 & Capture::IgnoreStdout as u8 != 0 {
                    if let Ok(null) = null_file.as_ref() {
                        let _ = unistd::dup2(null.as_raw_fd(), nix::libc::STDOUT_FILENO);
                    }
                } else if let Some((_, write)) = outs {
                    let _ = unistd::dup2(write.as_raw_fd(), nix::libc::STDOUT_FILENO);
                }

                // Redirect standard error to a pipe, or /dev/null, if needed.
                if self.capture as u8 & Capture::IgnoreStderr as u8 != 0 {
                    if let Ok(null) = null_file.as_ref() {
                        let _ = unistd::dup2(null.as_raw_fd(), nix::libc::STDERR_FILENO);
                    }
                } else if let Some((_, write)) = errs {
                    let _ = unistd::dup2(write.as_raw_fd(), nix::libc::STDERR_FILENO);
                }

                // Drop all the file descriptors that we no longer need.
                drop(null_file);

                // Obtain ownership of the child's copy of the shell, and then configure it.
                let mut shell: Shell<'b> = unsafe { (self.shell as *const Shell<'b>).read() };
                shell.variables_mut().set("PID", unistd::getpid().to_string());

                // Execute the given closure within the child's shell.
                if let Err(why) = child_func(&mut shell) {
                    eprintln!("{}", why);
                    unsafe { nix::libc::_exit(-1) };
                } else {
                    unsafe { nix::libc::_exit(shell.previous_status.as_os_code()) };
                }
            }
            ForkResult::Parent { child } => {
                Ok(IonResult {
                    child,
                    stdout: outs.map(|(read, write)| {
                        drop(write);
                        read
                    }),
                    stderr: errs.map(|(read, write)| {
                        drop(write);
                        read
                    }),
                    // `waitpid()` is required to reap the child.
                    status: wait_for_child(child)?,
                })
            }
        }
    }

    /// Creates a new `Fork` state from an existing shell.
    pub const fn new(shell: &'a Shell<'b>, capture: Capture) -> Self { Self { shell, capture } }
}
