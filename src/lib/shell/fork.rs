use super::{sys, IonError, Shell};
use std::{
    fs::File,
    io,
    os::unix::io::{AsRawFd, FromRawFd},
};

pub fn wait_for_child(pid: u32) -> io::Result<u8> {
    loop {
        let mut status = 0;
        if let Err(errno) = sys::waitpid(pid as i32, &mut status, libc::WUNTRACED) {
            break if errno == libc::ECHILD {
                Ok(sys::wexitstatus(status) as u8)
            } else {
                Err(io::Error::from_raw_os_error(errno))
            };
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
    pub pid:    u32,
    pub stdout: Option<File>,
    pub stderr: Option<File>,
    pub status: u8,
}

impl<'a, 'b> Fork<'a, 'b> {
    /// Executes a closure within the child of the fork, and returning an `IonResult` in a
    /// non-blocking fashion.
    pub fn exec<F: FnMut(&mut Shell<'b>) -> Result<(), IonError> + 'a>(
        self,
        mut child_func: F,
    ) -> Result<IonResult, IonError> {
        sys::signals::block();

        // If we are to capture stdout, create a pipe for capturing outputs.
        let outs = if self.capture as u8 & Capture::Stdout as u8 != 0 {
            let fds = sys::pipe2(libc::O_CLOEXEC)?;
            Some(unsafe { (File::from_raw_fd(fds.0), File::from_raw_fd(fds.1)) })
        } else {
            None
        };

        // And if we are to capture stderr, create a pipe for that as well.
        let errs = if self.capture as u8 & Capture::Stderr as u8 != 0 {
            let fds = sys::pipe2(libc::O_CLOEXEC)?;
            Some(unsafe { (File::from_raw_fd(fds.0), File::from_raw_fd(fds.1)) })
        } else {
            None
        };

        // TODO: Have a global static store a File that points to /dev/null (or :null for Redox)
        // at the beginning of the program so that any request for /dev/null's fd doesn't need to
        // be repeated.
        let null_file = File::open(sys::NULL_PATH);

        match unsafe { sys::fork() }? {
            0 => {
                // Allow the child to handle it's own signal handling.
                sys::signals::unblock();

                // Redirect standard output to a pipe, or /dev/null, if needed.
                if self.capture as u8 & Capture::IgnoreStdout as u8 != 0 {
                    if let Ok(null) = null_file.as_ref() {
                        let _ = sys::dup2(null.as_raw_fd(), libc::STDOUT_FILENO);
                    }
                } else if let Some((_, write)) = outs {
                    let _ = sys::dup2(write.as_raw_fd(), libc::STDOUT_FILENO);
                }

                // Redirect standard error to a pipe, or /dev/null, if needed.
                if self.capture as u8 & Capture::IgnoreStderr as u8 != 0 {
                    if let Ok(null) = null_file.as_ref() {
                        let _ = sys::dup2(null.as_raw_fd(), libc::STDERR_FILENO);
                    }
                } else if let Some((_, write)) = errs {
                    let _ = sys::dup2(write.as_raw_fd(), libc::STDERR_FILENO);
                }

                // Drop all the file descriptors that we no longer need.
                drop(null_file);

                // Obtain ownership of the child's copy of the shell, and then configure it.
                let mut shell: Shell<'b> = unsafe { (self.shell as *const Shell<'b>).read() };
                shell.variables_mut().set("PID", sys::getpid().unwrap_or(0).to_string());

                // Execute the given closure within the child's shell.
                if let Err(why) = child_func(&mut shell) {
                    eprintln!("{}", why);
                    sys::fork_exit(-1);
                } else {
                    sys::fork_exit(shell.previous_status.as_os_code());
                }
            }
            pid => {
                Ok(IonResult {
                    pid,
                    stdout: outs.map(|(read, write)| {
                        drop(write);
                        read
                    }),
                    stderr: errs.map(|(read, write)| {
                        drop(write);
                        read
                    }),
                    // `waitpid()` is required to reap the child.
                    status: wait_for_child(pid)?,
                })
            }
        }
    }

    /// Creates a new `Fork` state from an existing shell.
    pub const fn new(shell: &'a Shell<'b>, capture: Capture) -> Self { Self { shell, capture } }
}
