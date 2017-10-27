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
    /// Represents redirection into stdin from more than one source
    Cat {
        sources: Vec<File>,
        stdin:   Option<File>,
        stdout:  Option<File>,
    },
    Tee {
        /// 0 for stdout, 1 for stderr
        items: (Option<TeeItem>, Option<TeeItem>),
        stdin: Option<File>,
        stdout: Option<File>,
        stderr: Option<File>,
    },
}

pub struct TeeItem {
    /// Where to read from for this tee. Generally only necessary if we need to tee both
    /// stdout and stderr.
    pub source: Option<File>,
    pub sinks: Vec<File>,
}

impl TeeItem {
    /// Writes out to all destinations of a Tee. Takes an extra `RedirectFrom` argument in
    /// order to
    /// handle piping. `RedirectFrom` paradoxically indicates where we are piping **to**. It
    /// should
    /// never be `RedirectFrom`::Both`
    pub(crate) fn write_to_all(&mut self, extra: Option<RedirectFrom>) -> ::std::io::Result<()> {
        use std::io::{self, Read, Write};
        use std::os::unix::io::*;
        fn write_out<R>(source: &mut R, sinks: &mut [File]) -> io::Result<()>
            where R: Read
        {
            let mut buf = [0; 4096];
            loop {
                // TODO: Figure out how to not block on this read
                let len = source.read(&mut buf)?;
                if len == 0 {
                    return Ok(());
                }
                for file in sinks.iter_mut() {
                    let mut total = 0;
                    loop {
                        let wrote = file.write(&buf[total..len])?;
                        total += wrote;
                        if total == len {
                            break;
                        }
                    }
                }
            }
        }
        let stdout = io::stdout();
        let stderr = io::stderr();
        match extra {
            None => {}
            Some(RedirectFrom::Stdout) => unsafe {
                self.sinks.push(File::from_raw_fd(stdout.as_raw_fd()))
            },
            Some(RedirectFrom::Stderr) => unsafe {
                self.sinks.push(File::from_raw_fd(stderr.as_raw_fd()))
            },
            Some(RedirectFrom::Both) => {
                panic!("logic error! extra should never be RedirectFrom::Both")
            }
        };
        if let Some(ref mut file) = self.source {
            write_out(file, &mut self.sinks)
        } else {
            let stdin = io::stdin();
            let mut stdin = stdin.lock();
            write_out(&mut stdin, &mut self.sinks)
        }
    }
}

macro_rules! set_field {
    ($self:expr, $field:ident, $arg:expr) => {
        match *$self {
            RefinedJob::External(ref mut command) => {
                command.$field(Stdio::from($arg));
            }
            RefinedJob::Builtin { ref mut $field,  .. } |
                RefinedJob::Function { ref mut $field, .. } |
                RefinedJob::Tee { ref mut $field, .. } => {
                *$field = Some($arg);
            }
            // Do nothing for Cat
            _ => {}
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

    pub(crate) fn cat(sources: Vec<File>) -> Self {
        RefinedJob::Cat {
            sources,
            stdin: None,
            stdout: None,
        }
    }

    pub(crate) fn tee(tee_out: Option<TeeItem>, tee_err: Option<TeeItem>) -> Self {
        RefinedJob::Tee {
            items:  (tee_out, tee_err),
            stdin:  None,
            stdout: None,
            stderr: None,
        }
    }

    pub(crate) fn stdin(&mut self, file: File) {
        if let &mut RefinedJob::Cat { ref mut stdin, .. } = self {
            *stdin = Some(file);
        } else {
            set_field!(self, stdin, file);
        }
    }

    pub(crate) fn stdout(&mut self, file: File) {
        if let &mut RefinedJob::Cat { ref mut stdout, .. } = self {
            *stdout = Some(file);
        } else {
            set_field!(self, stdout, file);
        }
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
            // TODO: Print for real
            RefinedJob::Cat { .. } => "multi-input".into(),
            RefinedJob::Tee { .. } => "multi-output".into(),
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
            // TODO: Figure out real printing
            RefinedJob::Cat { .. } | RefinedJob::Tee { .. } => "".into(),
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
