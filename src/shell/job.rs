use std::fs::File;
use std::process::{Command, Stdio};

// use glob::glob;

use parser::{expand_string, Expander};
use parser::pipelines::RedirectFrom;
use smallstring::SmallString;
use types::*;

#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum JobKind {
    And,
    Background,
    Last,
    Or,
    Pipe(RedirectFrom),
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Job {
    pub command: Identifier,
    pub args:    Array,
    pub kind:    JobKind,
}

impl Job {
    pub(crate) fn new(args: Array, kind: JobKind) -> Self {
        let command = SmallString::from_str(&args[0]);
        Job {
            command,
            args,
            kind,
        }
    }

    /// Takes the current job's arguments and expands them, one argument at a
    /// time, returning a new `Job` with the expanded arguments.
    pub(crate) fn expand<E: Expander>(&mut self, expanders: &E) {
        let mut expanded = Array::new();
        expanded.grow(self.args.len());
        expanded.extend(self.args.drain().flat_map(|arg| {
            let res = expand_string(&arg, expanders, false);
            if res.is_empty() {
                array![""]
            } else {
                res
            }
        }));
        self.args = expanded;
    }
}

/// This represents a job that has been processed and expanded to be run
/// as part of some pipeline
pub(crate) enum RefinedJob {
    /// An external program that is executed by this shell
    External(Command),
    /// A procedure embedded into Ion
    Builtin {
        /// Name of the procedure
        name: Identifier,
        /// Arguments to pass in to the procedure
        args: Array,
        /// A file corresponding to the standard input for this builtin
        stdin: Option<File>,
        /// A file corresponding to the standard output for this builtin
        stdout: Option<File>,
        /// A file corresponding to the standard error for this builtin
        stderr: Option<File>,
    },
    /// Functions can act as commands too!
    Function {
        /// Name of the procedure
        name: Identifier,
        /// Arguments to pass in to the procedure
        args: Array,
        /// A file corresponding to the standard input for this builtin
        stdin: Option<File>,
        /// A file corresponding to the standard output for this builtin
        stdout: Option<File>,
        /// A file corresponding to the standard error for this builtin
        stderr: Option<File>,
    },
}

macro_rules! set_field {
    ($self:expr, $field:ident, $arg:expr) => {
        match *$self {
            RefinedJob::External(ref mut command) => {
                command.$field(Stdio::from($arg));
            }
            RefinedJob::Builtin { ref mut $field,  .. } | RefinedJob::Function { ref mut $field, .. } => {
                *$field = Some($arg);
            }
        }
    }
}

impl RefinedJob {
    pub(crate) fn builtin(name: Identifier, args: Array) -> Self {
        RefinedJob::Builtin {
            name,
            args,
            stdin: None,
            stdout: None,
            stderr: None,
        }
    }

    pub(crate) fn function(name: Identifier, args: Array) -> Self {
        RefinedJob::Function {
            name,
            args,
            stdin: None,
            stdout: None,
            stderr: None,
        }
    }

    pub(crate) fn stdin(&mut self, file: File) {
        set_field!(self, stdin, file);
    }

    pub(crate) fn stdout(&mut self, file: File) {
        set_field!(self, stdout, file);
    }

    pub(crate) fn stderr(&mut self, file: File) {
        set_field!(self, stderr, file);
    }

    /// Returns a short description of this job: often just the command
    /// or builtin name
    pub(crate) fn short(&self) -> String {
        match *self {
            RefinedJob::External(ref cmd) => {
                format!("{:?}", cmd).split('"').nth(1).unwrap_or("").to_string()
            }
            RefinedJob::Builtin { ref name, .. } | RefinedJob::Function { ref name, .. } => {
                name.to_string()
            }
        }
    }

    /// Returns a long description of this job: the commands and arguments
    pub(crate) fn long(&self) -> String {
        match *self {
            RefinedJob::External(ref cmd) => {
                let command = format!("{:?}", cmd);
                let mut arg_iter = command.split_whitespace();
                let command = arg_iter.next().unwrap();
                let mut output = String::from(&command[1..command.len() - 1]);
                for argument in arg_iter {
                    output.push(' ');
                    if argument.len() > 2 {
                        output.push_str(&argument[1..argument.len() - 1]);
                    } else {
                        output.push_str(&argument);
                    }
                }
                output
            }
            RefinedJob::Builtin { ref args, .. } | RefinedJob::Function { ref args, .. } => {
                format!("{}", args.join(" "))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parser::Expander;

    struct Empty;

    impl Expander for Empty {}

    #[test]
    fn preserve_empty_arg() {
        let job = Job::new(array!("rename", "", "0", "a"), JobKind::Last);
        let mut expanded = job.clone();
        expanded.expand(&Empty);
        assert_eq!(job, expanded);
    }

}
