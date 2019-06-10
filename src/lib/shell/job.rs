use super::Shell;
use crate::{
    builtins::BuiltinFunction,
    parser::{pipelines::RedirectFrom, Expander},
    types, Value,
};
use std::{fmt, fs::File, str};

#[derive(Clone)]
pub struct Job<'a> {
    pub args:        types::Args,
    pub redirection: RedirectFrom,
    pub builtin:     Option<BuiltinFunction<'a>>,
}

impl<'a> Job<'a> {
    /// Get the job command (its first arg)
    pub fn command(&self) -> &types::Str { &self.args[0] }

    /// Takes the current job's arguments and expands them, one argument at a
    /// time, returning a new `Job` with the expanded arguments.
    pub fn expand(&mut self, shell: &Shell) {
        match shell.variables.get_ref(&self.args[0]) {
            Some(Value::Function(_)) => {}
            _ => self.args = self.args.drain().flat_map(|arg| expand_arg(&arg, shell)).collect(),
        }
    }

    pub fn new(
        args: types::Args,
        redirection: RedirectFrom,
        builtin: Option<BuiltinFunction<'a>>,
    ) -> Self {
        Job { args, redirection, builtin }
    }
}

impl<'a> PartialEq for Job<'a> {
    fn eq(&self, other: &Job) -> bool {
        self.args == other.args && self.redirection == other.redirection
    }
}

impl<'a> fmt::Debug for Job<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Job {{ command: {}, args: {:?}, redirection: {:?} }}",
            self.args[0], self.args, self.redirection
        )
    }
}

/// Expands a given argument and returns it as an `Args`.
fn expand_arg(arg: &str, shell: &Shell) -> types::Args {
    let res = shell.expand_string(&arg);
    if res.is_empty() {
        args![""]
    } else {
        res
    }
}

/// This represents a job that has been processed and expanded to be run
/// as part of some pipeline
pub struct RefinedJob<'a> {
    pub stdin:  Option<File>,
    pub stdout: Option<File>,
    pub stderr: Option<File>,
    pub args:   types::Args,
    pub var:    JobVariant<'a>,
}

pub enum JobVariant<'a> {
    /// An external program that is executed by this shell
    External,
    /// A procedure embedded into Ion
    Builtin { main: BuiltinFunction<'a> },
    /// Functions can act as commands too!
    Function,
    /// Represents redirection into stdin from more than one source
    Cat { sources: Vec<File> },
    Tee {
        /// 0 for stdout, 1 for stderr
        items: (Option<TeeItem>, Option<TeeItem>),
    },
}

#[derive(Debug)]
pub struct TeeItem {
    /// Where to read from for this tee. Generally only necessary if we need to tee both
    /// stdout and stderr.
    pub source: Option<File>,
    pub sinks: Vec<File>,
}

impl TeeItem {
    pub fn new() -> Self { TeeItem { sinks: Vec::new(), source: None } }

    pub fn add(&mut self, sink: File) { self.sinks.push(sink); }

    /// Writes out to all destinations of a Tee. Takes an extra `RedirectFrom` argument in
    /// order to
    /// handle piping. `RedirectFrom` paradoxically indicates where we are piping **to**. It
    /// should
    /// never be `RedirectFrom`::Both`
    pub fn write_to_all(&mut self, extra: Option<RedirectFrom>) -> ::std::io::Result<()> {
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
                    file.write_all(&buf[..len])?;
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
            Some(RedirectFrom::None) => panic!("logic error! No need to tee if no redirections"),
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

impl<'a> RefinedJob<'a> {
    pub fn command(&self) -> &types::Str { &self.args[0] }

    pub fn args(&self) -> &types::Args { &self.args }

    pub fn stderr(&mut self, file: File) {
        if let JobVariant::Cat { .. } = self.var {
            return;
        }

        self.stderr = Some(file);
    }

    pub fn needs_forking(&self) -> bool {
        match self.var {
            JobVariant::Function | JobVariant::Builtin { .. } => false,
            _ => true,
        }
    }

    pub fn stdout(&mut self, file: File) { self.stdout = Some(file); }

    pub fn stdin(&mut self, file: File) { self.stdin = Some(file); }

    pub fn tee(tee_out: Option<TeeItem>, tee_err: Option<TeeItem>) -> Self {
        RefinedJob {
            stdin:  None,
            stdout: None,
            stderr: None,
            args:   types::Args::new(),
            var:    JobVariant::Tee { items: (tee_out, tee_err) },
        }
    }

    pub fn cat(sources: Vec<File>) -> Self {
        RefinedJob {
            stdin:  None,
            stdout: None,
            stderr: None,
            args:   types::Args::new(),
            var:    JobVariant::Cat { sources },
        }
    }

    pub fn function(args: types::Args) -> Self {
        RefinedJob { stdin: None, stdout: None, stderr: None, args, var: JobVariant::Function }
    }

    pub fn builtin(main: BuiltinFunction<'a>, args: types::Args) -> Self {
        RefinedJob {
            stdin: None,
            stdout: None,
            stderr: None,
            args,
            var: JobVariant::Builtin { main },
        }
    }

    pub fn external(args: types::Args) -> Self {
        RefinedJob { stdin: None, stdout: None, stderr: None, args, var: JobVariant::External }
    }
}
