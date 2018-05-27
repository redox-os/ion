use super::Shell;
use shell::pipe_exec::PipelineExecution;
use builtins::{BuiltinFunction, BUILTINS};
use parser::{expand_string, pipelines::RedirectFrom};
use smallstring::SmallString;
use smallvec::SmallVec;
use std::{fmt, fs::File, str};
use types::*;

#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum JobKind {
    Background,
    Disown,
    Last,
    Pipe(RedirectFrom),
}

#[derive(Clone)]
pub(crate) struct Job {
    pub command: Identifier,
    pub args:    Array,
    pub kind:    JobKind,
    pub builtin: Option<BuiltinFunction>,
}

impl Job {
    /// Takes the current job's arguments and expands them, one argument at a
    /// time, returning a new `Job` with the expanded arguments.
    pub(crate) fn expand(&mut self, shell: &Shell) {
        let mut expanded = Array::new();
        expanded.grow(self.args.len());
        expanded.extend(self.args.drain().flat_map(|arg| expand_arg(&arg, shell)));
        self.args = expanded;
    }

    pub(crate) fn new(args: Array, kind: JobKind) -> Self {
        let command = SmallString::from_str(&args[0]);
        let builtin = BUILTINS.get(command.as_ref()).map(|b| b.main);
        Job {
            command,
            args,
            kind,
            builtin,
        }
    }
}

impl PartialEq for Job {
    fn eq(&self, other: &Job) -> bool {
        self.command == other.command && self.args == other.args && self.kind == other.kind
    }
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Job {{ command: {}, args: {:?}, kind: {:?} }}",
            self.command, self.args, self.kind
        )
    }
}

/// Expands a given argument and returns it as an `Array`.
fn expand_arg(arg: &str, shell: &Shell) -> Array {
    let res = expand_string(&arg, shell, false);
    if res.is_empty() {
        array![""]
    } else {
        res
    }
}

/// This represents a job that has been processed and expanded to be run
/// as part of some pipeline
pub(crate) enum RefinedJob {
    /// An external program that is executed by this shell
    External {
        name:   Identifier,
        args:   Array,
        stdin:  Option<File>,
        stdout: Option<File>,
        stderr: Option<File>,
    },
    /// A procedure embedded into Ion
    Builtin {
        main:   BuiltinFunction,
        args:   Array,
        stdin:  Option<File>,
        stdout: Option<File>,
        stderr: Option<File>,
    },
    /// Functions can act as commands too!
    Function {
        name:   Identifier,
        args:   Array,
        stdin:  Option<File>,
        stdout: Option<File>,
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
        use std::{
            io::{self, Read, Write},
            os::unix::io::*,
        };
        fn write_out<R>(source: &mut R, sinks: &mut [File]) -> io::Result<()>
        where
            R: Read,
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
    ($self: expr, $field: ident, $arg: expr) => {
        match *$self {
            RefinedJob::External { ref mut $field, .. }
            | RefinedJob::Builtin { ref mut $field, .. }
            | RefinedJob::Function { ref mut $field, .. }
            | RefinedJob::Tee { ref mut $field, .. } => {
                *$field = Some($arg);
            }
            // Do nothing for Cat
            _ => {}
        }
    };
}

fn collect_args(args: &[String]) -> SmallVec<[&str; 16]> {
    args.iter().map(|x| x as &str).collect::<SmallVec<[&str; 16]>>()
}

impl RefinedJob {
    /// Returns a long description of this job: the commands and arguments
    pub(crate) fn long(&self) -> String {
        match *self {
            RefinedJob::External { ref args, .. }
            | RefinedJob::Builtin { ref args, .. }
            | RefinedJob::Function { ref args, .. } => format!("{}", args.join(" ")),
            // TODO: Figure out real printing
            RefinedJob::Cat { .. } | RefinedJob::Tee { .. } => "".into(),
        }
    }

    /// Returns a short description of this job: often just the command
    /// or builtin name
    pub(crate) fn short(&self) -> String {
        match *self {
            RefinedJob::Builtin { .. } => String::from("Shell Builtin"),
            RefinedJob::Function { ref name, .. } | RefinedJob::External { ref name, .. } => {
                name.to_string()
            }
            // TODO: Print for real
            RefinedJob::Cat { .. } => "multi-input".into(),
            RefinedJob::Tee { .. } => "multi-output".into(),
        }
    }

    pub(crate) fn exec<S: PipelineExecution>(&self, shell: &mut S) -> i32 {
        match *self {
            RefinedJob::External {
                ref name,
                ref args,
                ref stdin,
                ref stdout,
                ref stderr,
            } => {
                let args = collect_args(&args[1..]);
                shell.exec_external(&name, &args, stdin, stdout, stderr)
            }
            RefinedJob::Builtin {
                main,
                ref args,
                ref stdin,
                ref stdout,
                ref stderr,
            } => {
                let args = collect_args(&args);
                shell.exec_builtin(main, &args, stdout, stderr, stdin)
            }
            RefinedJob::Function {
                ref name,
                ref args,
                ref stdin,
                ref stdout,
                ref stderr,
            } => {
                let args = collect_args(&args);
                shell.exec_function(name, &args, stdout, stderr, stdin)
            }
            _ => panic!("exec job should not be able to be called on Cat or Tee jobs"),
        }
    }

    pub(crate) fn stderr(&mut self, file: File) {
        set_field!(self, stderr, file);
    }

    pub(crate) fn stdout(&mut self, file: File) {
        if let &mut RefinedJob::Cat { ref mut stdout, .. } = self {
            *stdout = Some(file);
        } else {
            set_field!(self, stdout, file);
        }
    }

    pub(crate) fn stdin(&mut self, file: File) {
        if let &mut RefinedJob::Cat { ref mut stdin, .. } = self {
            *stdin = Some(file);
        } else {
            set_field!(self, stdin, file);
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

    pub(crate) fn cat(sources: Vec<File>) -> Self {
        RefinedJob::Cat {
            sources,
            stdin: None,
            stdout: None,
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

    pub(crate) fn builtin(main: BuiltinFunction, args: Array) -> Self {
        RefinedJob::Builtin {
            main,
            args,
            stdin: None,
            stdout: None,
            stderr: None,
        }
    }

    pub(crate) fn external(name: Identifier, args: Array) -> Self {
        RefinedJob::External {
            name,
            args,
            stdin: None,
            stdout: None,
            stderr: None,
        }
    }
}
