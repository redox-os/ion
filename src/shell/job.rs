use std::process::{Command, Stdio};
use std::os::unix::io::{RawFd, FromRawFd};

//use glob::glob;
use parser::{expand_string, ExpanderFunctions};
use parser::peg::RedirectFrom;
use smallstring::SmallString;
use sys;
use types::*;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum JobKind { And, Background, Last, Or, Pipe(RedirectFrom) }

#[derive(Debug, PartialEq, Clone)]
pub struct Job {
    pub command: Identifier,
    pub args: Array,
    pub kind: JobKind,
}

impl Job {
    pub fn new(args: Array, kind: JobKind) -> Self {
        let command = SmallString::from_str(&args[0]);
        Job { command, args, kind }
    }

    /// Takes the current job's arguments and expands them, one argument at a
    /// time, returning a new `Job` with the expanded arguments.
    pub fn expand(&mut self, expanders: &ExpanderFunctions) {
        let mut expanded = Array::new();
        expanded.grow(self.args.len());
        expanded.extend(self.args.drain().flat_map(|arg| {
            expand_string(&arg, expanders, false)
        }));
        self.args = expanded;
    }

}

/// This represents a job that has been processed and expanded to be run
/// as part of some pipeline
pub enum RefinedJob {
    /// An external program that is executed by this shell
    External(Command),
    /// A procedure embedded into Ion
    Builtin {
        /// Name of the procedure
        name: Identifier,
        /// Arguments to pass in to the procedure
        args: Array,
        /// A file corresponding to the standard input for this builtin
        stdin: Option<RawFd>,
        /// A file corresponding to the standard output for this builtin
        stdout: Option<RawFd>,
        /// A file corresponding to the standard error for this builtin
        stderr: Option<RawFd>,
    }
}

macro_rules! set_field {
    ($self:expr, $field:ident, $arg:expr) => {
        match *$self {
            RefinedJob::External(ref mut command) => {
                unsafe {
                    command.$field(Stdio::from_raw_fd($arg));
                }
            }
            RefinedJob::Builtin { ref mut $field,  .. } => {
                *$field = Some($arg);
            }
        }
    }
}

impl RefinedJob {

    pub fn builtin(name: Identifier, args: Array) -> Self {
        RefinedJob::Builtin {
            name,
            args,
            stdin: None,
            stdout: None,
            stderr: None
        }
    }

    pub fn stdin(&mut self, fd: RawFd) {
        set_field!(self, stdin, fd);
    }

    pub fn stdout(&mut self, fd: RawFd) {
        set_field!(self, stdout, fd);
    }

    pub fn stderr(&mut self, fd: RawFd) {
        set_field!(self, stderr, fd);
    }

    /// Returns a short description of this job: often just the command
    /// or builtin name
    pub fn short(&self) -> String {
        match *self {
            RefinedJob::External(ref cmd) => {
                format!("{:?}", cmd).split('"').nth(1).unwrap_or("").to_string()
            },
            RefinedJob::Builtin { ref name, .. } => {
                name.to_string()
            }
        }
    }

    /// Returns a long description of this job: the commands and arguments
    pub fn long(&self) -> String {
        match *self {
            RefinedJob::External(ref cmd) => {
                let command = format!("{:?}", cmd);
                let mut arg_iter = command.split_whitespace();
                let command = arg_iter.next().unwrap();
                let mut output = String::from(&command[1..command.len()-1]);
                for argument in arg_iter {
                    output.push(' ');
                    if argument.len() > 2 {
                        output.push_str(&argument[1..argument.len()-1]);
                    } else {
                        output.push_str(&argument);
                    }
                }
                output
            },
            RefinedJob::Builtin { ref args, .. } => {
                format!("{}", args.join(" "))
            }
        }
    }

}

impl Drop for RefinedJob {

    // This is needed in order to ensure that the parent instance of RefinedJob
    // cleans up after its own `RawFd`s; otherwise these would never be properly
    // closed, never sending EOF, causing any process reading from these
    // `RawFd`s to halt indefinitely.
    fn drop(&mut self) {
        match *self {
            RefinedJob::External(ref mut cmd) => {
                drop(cmd);
            },
            RefinedJob::Builtin {
                ref mut name,
                ref mut args,
                ref mut stdin,
                ref mut stdout,
                ref mut stderr,
            } => {
                fn close(fd: Option<RawFd>) {
                    if let Some(fd) = fd {
                        if let Err(e) = sys::close(fd) {
                            eprintln!("ion: failed to close file '{}': {}", fd, e);
                        }
                    }
                }
                drop(name);
                drop(args);
                close(*stdin);
                close(*stdout);
                close(*stderr);
            }
        }
    }

}
