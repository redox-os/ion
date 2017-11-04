use super::{Binary, FlowLogic, Shell};
use super::status::*;
use parser::QuoteTerminator;
use std::fmt::{self, Display, Formatter};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

pub enum IonError {
    Fork(io::Error),
}

impl Display for IonError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match *self {
            IonError::Fork(ref why) => writeln!(fmt, "failed to fork: {}", why),
        }
    }
}

pub struct IonResult {
    pub stdout: File,
    pub stderr: File,
}

pub trait IonLibrary {
    /// Executes the given command and returns the exit status, or if the supplied command is
    /// not
    /// terminated, an error will be returned.
    fn execute_command<CMD>(&mut self, command: CMD) -> Result<i32, &'static str>
        where CMD: Into<QuoteTerminator>;

    /// Executes all of the statements contained within a given script,
    /// returning the final exit status.
    fn execute_script<P: AsRef<Path>>(&mut self, path: P) -> io::Result<i32>;

    /// Performs a fork, taking a closure that controls the shell in the child of the fork.
    /// The method is non-blocking, and therefore will immediately return file handles to
    /// the stdout and stderr of the child.
    fn fork<F>(&self, child_func: F) -> Result<IonResult, IonError>
        where F: FnMut(&mut Shell);
}

impl IonLibrary for Shell {
    fn execute_command<CMD>(&mut self, command: CMD) -> Result<i32, &'static str>
        where CMD: Into<QuoteTerminator>
    {
        let mut terminator = command.into();
        if terminator.check_termination() {
            self.on_command(&terminator.consume());
            Ok(self.previous_status)
        } else {
            Err("input is not terminated")
        }
    }

    fn execute_script<P: AsRef<Path>>(&mut self, path: P) -> io::Result<i32> {
        let mut file = File::open(path.as_ref())?;
        let capacity = file.metadata().ok().map_or(0, |x| x.len());
        let mut command_list = String::with_capacity(capacity as usize);
        let _ = file.read_to_string(&mut command_list)?;
        if FAILURE == self.terminate_script_quotes(command_list.lines().map(|x| x.to_owned())) {
            self.previous_status = FAILURE;
        }
        Ok(self.previous_status)
    }

    fn fork<F: FnMut(&mut Shell)>(&self, mut child_func: F) -> Result<IonResult, IonError> {
        use std::os::unix::io::{AsRawFd, FromRawFd};
        use std::process::exit;
        use sys;

        let (stdout_read, stdout_write) = sys::pipe2(sys::O_CLOEXEC)
            .map(|fds| unsafe { (File::from_raw_fd(fds.0), File::from_raw_fd(fds.1)) })
            .map_err(IonError::Fork)?;

        let (stderr_read, stderr_write) = sys::pipe2(sys::O_CLOEXEC)
            .map(|fds| unsafe { (File::from_raw_fd(fds.0), File::from_raw_fd(fds.1)) })
            .map_err(IonError::Fork)?;

        match unsafe { sys::fork() } {
            Ok(0) => {
                let _ = sys::dup2(stdout_write.as_raw_fd(), sys::STDOUT_FILENO);
                let _ = sys::dup2(stderr_write.as_raw_fd(), sys::STDERR_FILENO);

                drop(stdout_write);
                drop(stdout_read);
                drop(stderr_write);
                drop(stderr_read);

                let mut shell: Shell = unsafe { (self as *const Shell).read() };
                child_func(&mut shell);

                // Reap the child, enabling the parent to get EOF from the read end of the pipe.
                exit(shell.previous_status);
            }
            Ok(_pid) => {
                // Drop the write end of the pipe, because the parent will not use it.
                drop(stdout_write);
                drop(stderr_write);

                Ok(IonResult {
                    stdout: stdout_read,
                    stderr: stderr_read,
                })
            }
            Err(why) => Err(IonError::Fork(why)),
        }
    }
}
