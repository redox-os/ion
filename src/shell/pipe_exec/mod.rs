//! The purpose of the pipeline execution module is to create commands from supplied pieplines, and
//! manage their execution thereof. That includes forking, executing commands, managing process
//! group
//! IDs, watching foreground and background tasks, sending foreground tasks to the background,
//! handling pipeline and conditional operators, and std{in,out,err} redirections.

pub mod foreground;
mod fork;
pub mod job_control;

use self::fork::{create_process_group, fork_pipe};
use self::job_control::JobControl;
use super::{JobKind, Shell};
use super::flags::*;
use super::flow_control::FunctionError;
use super::job::{RefinedJob, TeeItem};
use super::signals::{self, SignalHandler};
use super::status::*;
use parser::pipelines::{Input, PipeItem, Pipeline, RedirectFrom, Redirection};
use std::fs::{File, OpenOptions};
use std::io::{self, Error, Write};
use std::iter;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{exit, Command};
use sys;

type RefinedItem = (RefinedJob, JobKind, Vec<Redirection>, Vec<Input>);

/// Use dup2 to replace `old` with `new` using `old`s file descriptor ID
fn redir(old: RawFd, new: RawFd) {
    if let Err(e) = sys::dup2(old, new) {
        eprintln!("ion: could not duplicate {} to {}: {}", old, new, e);
    }
}

/// Create an OS pipe and write the contents of a byte slice to one end
/// such that reading from this pipe will produce the byte slice. Return
/// A file descriptor representing the read end of the pipe.
pub unsafe fn stdin_of<T: AsRef<[u8]>>(input: T) -> Result<RawFd, Error> {
    let (reader, writer) = sys::pipe2(sys::O_CLOEXEC)?;
    let mut infile = File::from_raw_fd(writer);
    // Write the contents; make sure to use write_all so that we block until
    // the entire string is written
    infile.write_all(input.as_ref())?;
    infile.flush()?;
    // `infile` currently owns the writer end RawFd. If we just return the reader end
    // and let `infile` go out of scope, it will be closed, sending EOF to the reader!
    Ok(reader)
}

/// This function serves three purposes:
/// 1. If the result is `Some`, then we will fork the pipeline executing into the background.
/// 2. The value stored within `Some` will be that background job's command name.
/// 3. If `set -x` was set, print the command.
fn gen_background_string(pipeline: &Pipeline, print_comm: bool) -> Option<String> {
    if pipeline.items[pipeline.items.len() - 1].job.kind == JobKind::Background {
        let command = pipeline.to_string();
        if print_comm {
            eprintln!("> {}", command);
        }
        Some(command)
    } else if print_comm {
        eprintln!("> {}", pipeline.to_string());
        None
    } else {
        None
    }
}

/// Determines if the supplied command implicitly defines to change the directory.
///
/// This is detected by first checking if the argument starts with a '.' or an '/', or ends
/// with a '/'. If that validates, then it will check if the supplied argument is a valid
/// directory path.
#[inline(always)]
fn is_implicit_cd(argument: &str) -> bool {
    (argument.starts_with('.') || argument.starts_with('/') || argument.ends_with('/'))
        && Path::new(argument).is_dir()
}

/// This function is to be executed when a stdin value is supplied to a pipeline job.
///
/// Using that value, the stdin of the first command will be mapped to either a `File`,
/// or `HereString`, which may be either a herestring or heredoc. Returns `true` if
/// the input error occurred.
#[allow(dead_code)]
fn redirect_input(piped_commands: &mut Vec<RefinedItem>) -> bool {
    // let mut prev_kind = None; // TODO: Piping?
    for &mut (ref mut job, ref _kind, _, ref mut inputs) in piped_commands {
        // TODO: Determine if we need to care about piping
        match inputs.len() {
            0 => continue,
            1 => {
                match inputs[0] {
                    Input::File(ref filename) => match File::open(filename) {
                        Ok(file) => job.stdin(file),
                        Err(e) => {
                            eprintln!("ion: failed to redirect '{}' to stdin: {}", filename, e);
                            return true;
                        }
                    },
                    Input::HereString(ref mut string) => match unsafe { stdin_of(&string) } {
                        Ok(stdio) => job.stdin(unsafe { File::from_raw_fd(stdio) }),
                        Err(e) => {
                            eprintln!("ion: failed to redirect herestring '{}' to stdin: {}",
                                      string, e);
                            return true;
                        }
                    }
                }
            },
            _ => {
                match inputs[0] {
                    Input::File(ref filename) => match File::open(filename) {
                        Ok(file) => job.stdin(file),
                        Err(e) => {
                            eprintln!("ion: failed to redirect '{}' to stdin: {}", filename, e);
                            return true;
                        }
                    },
                    Input::HereString(ref mut string) => match unsafe { stdin_of(&string) } {
                        Ok(stdio) => job.stdin(unsafe { File::from_raw_fd(stdio) }),
                        Err(e) => {
                            eprintln!("ion: failed to redirect herestring '{}' to stdin: {}",
                                      string, e);
                            return true;
                        }
                    }
                }
            }
        }
        // prev_kind = Some(*kind); TODO: Piping
    }
    false
}

/// Insert the multiple redirects as pipelines if necessary. 
fn do_redirection(piped_commands: Vec<RefinedItem>)
    -> Option<Vec<(RefinedJob, JobKind)>>
{
    macro_rules! get_infile {
        ($input:expr) => {
            match $input {
                Input::File(ref filename) => match File::open(filename) {
                    Ok(file) => Some(file),
                    Err(e) => {
                        eprintln!("ion: failed to redirect '{}' to stdin: {}", filename, e);
                        None
                    }
                },
                Input::HereString(ref mut string) => match unsafe { stdin_of(&string) } {
                    Ok(stdio) => Some(unsafe { File::from_raw_fd(stdio) }),
                    Err(e) => {
                        eprintln!("ion: failed to redirect herestring '{}' to stdin: {}",
                                  string, e);
                        None
                    }
                }
            }
        }
    }

    let need_tee = |outs: &[_], kind| {
        let (mut stdout_count, mut stderr_count) = (0, 0);
        match kind {
            JobKind::Pipe(RedirectFrom::Both) => {
                stdout_count += 1;
                stderr_count += 1;
            },
            JobKind::Pipe(RedirectFrom::Stdout) => stdout_count += 1,
            JobKind::Pipe(RedirectFrom::Stderr) => stderr_count += 1,
            _ => {}
        }
        for out in outs {
            let &Redirection { from, .. } = out;
            match from {
                RedirectFrom::Both => {
                    stdout_count += 1;
                    stderr_count += 1;
                }
                RedirectFrom::Stdout => stdout_count += 1,
                RedirectFrom::Stderr => stderr_count += 1,
            }
            if stdout_count >= 2 && stderr_count >= 2 {
                return (true, true);
            }
        }
        (stdout_count >= 2, stderr_count >= 2)
    };

    macro_rules! set_no_tee {
        ($outputs:ident, $job:ident) => {
            // XXX: Possibly add an assertion here for correctness
            for output in $outputs {
                match if output.append {
                    OpenOptions::new().create(true).write(true).append(true).open(&output.file)
                } else {
                    File::create(&output.file)
                } {
                    Ok(f) => match output.from {
                        RedirectFrom::Stderr => $job.stderr(f),
                        RedirectFrom::Stdout => $job.stdout(f),
                        RedirectFrom::Both => match f.try_clone() {
                            Ok(f_copy) => {
                                $job.stdout(f);
                                $job.stderr(f_copy);
                            },
                            Err(e) => {
                                eprintln!(
                                    "ion: failed to redirect both stdout and stderr to file '{:?}': {}",
                                    f,
                                    e);
                                return None;
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("ion: failed to redirect output into {}: {}", output.file, e);
                        return None;
                    }
                }
            }
        }
    }

    macro_rules! set_one_tee {
        ($new:ident, $outputs:ident, $job:ident, $kind:ident, $teed:ident, $other:ident) => {{
            let mut tee = TeeItem { sinks: Vec::new(), source: None };
            for output in $outputs {
                match if output.append {
                    OpenOptions::new().create(true).write(true).append(true).open(&output.file)
                } else {
                    File::create(&output.file)
                } {
                    Ok(f) => match output.from {
                        RedirectFrom::$teed => tee.sinks.push(f),
                        RedirectFrom::$other => if RedirectFrom::Stdout == RedirectFrom::$teed {
                            $job.stderr(f);
                        } else {
                            $job.stdout(f);
                        },
                        RedirectFrom::Both => match f.try_clone() {
                            Ok(f_copy) => {
                                if RedirectFrom::Stdout == RedirectFrom::$teed {
                                    $job.stderr(f);
                                } else {
                                    $job.stdout(f);
                                }
                                tee.sinks.push(f_copy);
                            },
                            Err(e) => {
                                eprintln!(
                                    "ion: failed to redirect both stdout and stderr to file '{:?}': {}",
                                    f,
                                    e);
                                return None;
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("ion: failed to redirect output into {}: {}", output.file, e);
                        return None;
                    }
                }
            }
            $new.push(($job, JobKind::Pipe(RedirectFrom::$teed)));
            let items = if RedirectFrom::Stdout == RedirectFrom::$teed {
                [Some(tee), None]
            } else {
                [None, Some(tee)]
            };
            let tee = RefinedJob::Tee {
                items: items,
                stdin: None,
                stdout: None,
                stderr: None,
            };
            $new.push((tee, $kind));
        }}
    }


    // Real logic begins here
    let mut new_commands = Vec::new();
    let mut prev_kind = JobKind::And;
    for (mut job, kind, outputs, mut inputs) in piped_commands {
        match (inputs.len(), prev_kind) {
            (0, _) => {},
            (1, JobKind::Pipe(_)) => {
                let sources = vec![get_infile!(inputs[0])?];
                new_commands.push((RefinedJob::cat(sources, true),
                                   JobKind::Pipe(RedirectFrom::Stdout)));
            },
            (1, _) => job.stdin(get_infile!(inputs[0])?),
            (_, p) => {
                let mut sources = Vec::new();
                for mut input in inputs {
                    sources.push(if let Some(f) = get_infile!(input) {
                        f
                    } else {
                        return None;
                    });
                }
                let piped = if let JobKind::Pipe(_) = p { true } else { false };
                new_commands.push((RefinedJob::cat(sources, piped),
                                   JobKind::Pipe(RedirectFrom::Stdout)));
            },
        }
        prev_kind = kind;
        if outputs.is_empty() {
            new_commands.push((job, kind));
            continue;
        }
        match need_tee(&outputs, kind) {
            // No tees
            (false, false) => {
                set_no_tee!(outputs, job);
                new_commands.push((job, kind));
            }
            // tee stderr
            (false, true) => set_one_tee!(new_commands, outputs, job, kind, Stderr, Stdout),
            // tee stdout
            (true, false) => set_one_tee!(new_commands, outputs, job, kind, Stdout, Stderr),
            // tee both
            (true, true) => {
                let mut tee_out = TeeItem { sinks: Vec::new(), source: None };
                let mut tee_err = TeeItem { sinks: Vec::new(), source: None };
                for output in outputs {
                    match if output.append {
                        OpenOptions::new().create(true).write(true).append(true).open(&output.file)
                    } else {
                        File::create(&output.file)
                    } {
                        Ok(f) => match output.from {
                            RedirectFrom::Stdout => tee_out.sinks.push(f),
                            RedirectFrom::Stderr => tee_err.sinks.push(f),
                            RedirectFrom::Both => match f.try_clone() {
                                Ok(f_copy) => {
                                    tee_out.sinks.push(f);
                                    tee_err.sinks.push(f_copy);
                                },
                                Err(e) => {
                                    eprintln!(
                                        "ion: failed to redirect both stdout and stderr to file '{:?}': {}",
                                        f,
                                        e);
                                    return None;
                                }
                            }
                        },
                        Err(e) => {
                            eprintln!("ion: failed to redirect output into {}: {}", output.file, e);
                            return None;
                        }
                    }
                }
                let tee = RefinedJob::Tee {
                    items: [Some(tee_out), Some(tee_err)],
                    stdin: None,
                    stdout: None,
                    stderr: None,
                };
                new_commands.push((job, JobKind::Pipe(RedirectFrom::Stdout)));
                new_commands.push((tee, kind));
            }
        }
    }
    Some(new_commands)
}

/// This function is to be executed when a stdout/stderr value is supplied to a pipeline job.
///
/// Using that value, the stdout and/or stderr of the last command will be redirected accordingly
/// to the designated output. Returns `true` if the outputs couldn't be redirected.
#[allow(dead_code)]
fn redirect_output(piped_commands: &mut Vec<RefinedItem>) -> bool {
    // TODO: Redirect more than just the last one.
    for &mut (ref mut job, ref _kind, ref outputs, _) in piped_commands {
        for output in outputs {
            match if output.append {
                OpenOptions::new().create(true).write(true).append(true).open(&output.file)
            } else {
                File::create(&output.file)
            } {
                Ok(f) => match output.from {
                    RedirectFrom::Stderr => job.stderr(f),
                    RedirectFrom::Stdout => job.stdout(f),
                    RedirectFrom::Both => match f.try_clone() {
                        Ok(f_copy) => {
                            job.stdout(f);
                            job.stderr(f_copy);
                        },
                        Err(e) => {
                            eprintln!(
                                "ion: failed to redirect both stdout and stderr to file '{:?}': {}",
                                f,
                                e);
                            return true;
                        }
                    }
                },
                Err(e) => {
                    eprintln!("ion: failed to redirect output into {}: {}", output.file, e);
                    return true;
                }
            }
        }
    }
    false
}

pub(crate) trait PipelineExecution {
    /// Given a pipeline, generates commands and executes them.
    ///
    /// The `Pipeline` structure contains a vector of `Job`s, and redirections to perform on the
    /// pipeline. Executing a pipeline involves creating a vector of commands, of which each
    /// command may refer to either a builtin command, or an external command. These commands
    /// will then be sent to an internal `pipe` function for execution.
    ///
    /// Depending on which operators are supplied, jobs may conditionally execute, pipe their
    /// outputs to adjacent jobs in the pipeline, or execute in the background. To enable job
    /// control, these jobs will also be assigned to their own unique process groups, may be
    /// given foreground terminal access, and will be monitored for status changes in the event
    /// that a job was signaled to stop or killed.
    ///
    /// If a job is stopped, the shell will add that job to a list of background jobs and
    /// continue
    /// to watch the job in the background, printing notifications on status changes of that job
    /// over time.
    fn execute_pipeline(&mut self, pipeline: &mut Pipeline) -> i32;

    /// Generates a vector of commands from a given `Pipeline`.
    ///
    /// Each generated command will either be a builtin or external command, and will be
    /// associated will be marked as an `&&`, `||`, `|`, or final job.
    fn generate_commands(&self, pipeline: &mut Pipeline)
        -> Result<Vec<RefinedItem>, i32>;

    /// Waits for all of the children within a pipe to finish exuecting, returning the
    /// exit status of the last process in the queue.
    fn wait(&mut self, children: Vec<u32>, commands: Vec<RefinedJob>) -> i32;

    /// Executes a `RefinedJob` that was created in the `generate_commands` method.
    ///
    /// The aforementioned `RefinedJob` may be either a builtin or external command.
    /// The purpose of this function is therefore to execute both types accordingly.
    fn exec_job(&mut self, job: &mut RefinedJob, foreground: bool) -> i32;

    /// Execute a builtin in the current process.
    /// # Args
    /// * `shell`: A `Shell` that forwards relevant information to the builtin
    /// * `name`: Name of the builtin to execute.
    /// * `stdin`, `stdout`, `stderr`: File descriptors that will replace the
    ///    respective standard streams if they are not `None`
    /// # Preconditions
    /// * `shell.builtins.contains_key(name)`; otherwise this function will panic
    fn exec_builtin(
        &mut self,
        name: &str,
        args: &[&str],
        stdout: &Option<File>,
        stderr: &Option<File>,
        stdin: &Option<File>,
    ) -> i32;

    fn exec_function(
        &mut self,
        name: &str,
        args: &[&str],
        stdout: &Option<File>,
        stderr: &Option<File>,
        stdin: &Option<File>,
    ) -> i32;

    /// For cat jobs
    fn exec_multi_in(&mut self,
                     sources: &mut [File],
                     piped: bool,
                     stdin: &Option<File>,
                     stdout: &Option<File>,
    ) -> i32;
}

impl<'a> PipelineExecution for Shell<'a> {
    fn execute_pipeline(&mut self, pipeline: &mut Pipeline) -> i32 {
        // Remove any leftover foreground tasks from the last execution.
        self.foreground.clear();
        // If the supplied pipeline is a background, a string representing the command will be
        // stored here.
        let possible_background_name =
            gen_background_string(&pipeline, self.flags & PRINT_COMMS != 0);
        // Generates commands for execution, differentiating between external and builtin commands.
        let piped_commands = match self.generate_commands(pipeline) {
            Ok(commands) => commands,
            Err(error) => return error,
        };

        // Don't execute commands when the `-n` flag is passed.
        if self.flags & NO_EXEC != 0 {
            return SUCCESS;
        }

        let mut piped_commands = if let Some(c) = do_redirection(piped_commands) {
            c
        } else {
            return COULD_NOT_EXEC;
        };

        // If the given pipeline is a background task, fork the shell.
        if let Some(command_name) = possible_background_name {
            fork_pipe(self, piped_commands, command_name)
        } else {
            // While active, the SIGTTOU signal will be ignored.
            let _sig_ignore = SignalHandler::new();
            let foreground = !self.is_background_shell;
            // Execute each command in the pipeline, giving each command the foreground.
            let exit_status = pipe(self, piped_commands, foreground);
            // Set the shell as the foreground process again to regain the TTY.
            if foreground && !self.is_library {
                let _ = sys::tcsetpgrp(0, sys::getpid().unwrap());
            }
            exit_status
        }
    }

    fn generate_commands(
        &self,
        pipeline: &mut Pipeline,
    ) -> Result<Vec<RefinedItem>, i32> {
        let mut results = Vec::new();
        for item in pipeline.items.drain(..) {
            let PipeItem { mut job, outputs, inputs } = item;
            let refined = {
                if is_implicit_cd(&job.args[0]) {
                    RefinedJob::builtin(
                        "cd".into(),
                        iter::once("cd".into()).chain(job.args.drain()).collect(),
                    )
                } else if Path::new(&job.args[0]).is_dir() {
                    eprintln!("ion: cannot execute directory as command");
                    return Err(FAILURE);
                } else if self.functions.contains_key::<str>(job.args[0].as_ref()) {
                    RefinedJob::function(job.args[0].clone().into(), job.args.drain().collect())
                } else if self.builtins.contains_key::<str>(job.args[0].as_ref()) {
                    RefinedJob::builtin(job.args[0].clone().into(), job.args.drain().collect())
                } else {
                    let mut command = Command::new(job.args[0].clone());
                    for arg in job.args.drain().skip(1) {
                        command.arg(arg);
                    }
                    RefinedJob::External(command)
                }
            };
            results.push((refined, job.kind, outputs, inputs));
        }
        Ok(results)
    }

    fn wait(&mut self, mut children: Vec<u32>, mut commands: Vec<RefinedJob>) -> i32 {
        // TODO: Find a way to only do this when absolutely necessary.
        let as_string = commands.iter().map(RefinedJob::long).collect::<Vec<String>>().join(" | ");

        // Each process in the pipe has the same PGID, which is the first process's PID.
        let pgid = children[0];

        // If the last process exits, we know that all processes should exit.
        let last_pid = children[children.len() - 1];

        // Watch the foreground group, dropping all commands that exit as they exit.
        self.watch_foreground(
            pgid,
            last_pid,
            move || as_string,
            move |pid| if let Some(id) = children.iter().position(|&x| x as i32 == pid) {
                commands.remove(id);
                children.remove(id);
            },
        )
    }

    fn exec_job(&mut self, job: &mut RefinedJob, foreground: bool) -> i32 {
        let short = job.short();
        let long = job.long();
        match *job {
            RefinedJob::External(ref mut command) => match {
                command
                    .before_exec(move || {
                        signals::unblock();
                        create_process_group(0);
                        Ok(())
                    })
                    .spawn()
            } {
                Ok(child) => {
                    if foreground && !self.is_library {
                        let _ = sys::tcsetpgrp(0, child.id());
                    }
                    self.watch_foreground(child.id(), child.id(), move || long, |_| ())
                }
                Err(e) => if e.kind() == io::ErrorKind::NotFound {
                    eprintln!("ion: command not found: {}", short);
                    NO_SUCH_COMMAND
                } else {
                    eprintln!("ion: error spawning process: {}", e);
                    COULD_NOT_EXEC
                },
            },
            RefinedJob::Builtin {
                ref name,
                ref args,
                ref stdin,
                ref stdout,
                ref stderr,
            } => {
                if let Ok(stdout_bk) = sys::dup(sys::STDOUT_FILENO) {
                    if let Ok(stderr_bk) = sys::dup(sys::STDERR_FILENO) {
                        if let Ok(stdin_bk) = sys::dup(sys::STDIN_FILENO) {
                            let args: Vec<&str> = args.iter().map(|x| x as &str).collect();
                            let code = self.exec_builtin(name, &args, stdout, stderr, stdin);
                            redir(stdout_bk, sys::STDOUT_FILENO);
                            redir(stderr_bk, sys::STDERR_FILENO);
                            redir(stdin_bk, sys::STDIN_FILENO);
                            return code;
                        }
                        let _ = sys::close(stderr_bk);
                    }
                    let _ = sys::close(stdout_bk);
                }
                eprintln!("ion: failed to `dup` STDOUT, STDIN, or STDERR: not running '{}'", long);
                COULD_NOT_EXEC
            }
            RefinedJob::Function {
                ref name,
                ref args,
                ref stdin,
                ref stdout,
                ref stderr,
            } => {
                if let Ok(stdout_bk) = sys::dup(sys::STDOUT_FILENO) {
                    if let Ok(stderr_bk) = sys::dup(sys::STDERR_FILENO) {
                        if let Ok(stdin_bk) = sys::dup(sys::STDIN_FILENO) {
                            let args: Vec<&str> = args.iter().map(|x| x as &str).collect();
                            let code = self.exec_function(name, &args, stdout, stderr, stdin);
                            redir(stdout_bk, sys::STDOUT_FILENO);
                            redir(stderr_bk, sys::STDERR_FILENO);
                            redir(stdin_bk, sys::STDIN_FILENO);
                            return code;
                        }
                        let _ = sys::close(stderr_bk);
                    }
                    let _ = sys::close(stdout_bk);
                }
                eprintln!("ion: failed to `dup` STDOUT, STDIN, or STDERR: not running '{}'", long);
                COULD_NOT_EXEC
            }
            _ => panic!("exec job should not be able to be called on Cat or Tee jobs"),
        }
    }

    fn exec_builtin(
        &mut self,
        name: &str,
        args: &[&str],
        stdout: &Option<File>,
        stderr: &Option<File>,
        stdin: &Option<File>,
    ) -> i32 {
        if let Some(ref file) = *stdin {
            redir(file.as_raw_fd(), sys::STDIN_FILENO);
        }
        if let Some(ref file) = *stdout {
            redir(file.as_raw_fd(), sys::STDOUT_FILENO);
        }
        if let Some(ref file) = *stderr {
            redir(file.as_raw_fd(), sys::STDERR_FILENO);
        }
        // The precondition for this function asserts that there exists some `builtin`
        // in the shell named `name`, so we unwrap here.
        let builtin = self.builtins.get(name).unwrap();
        (builtin.main)(args, self)
    }

    fn exec_function(
        &mut self,
        name: &str,
        args: &[&str],
        stdout: &Option<File>,
        stderr: &Option<File>,
        stdin: &Option<File>,
    ) -> i32 {
        if let Some(ref file) = *stdin {
            redir(file.as_raw_fd(), sys::STDIN_FILENO);
        }
        if let Some(ref file) = *stdout {
            redir(file.as_raw_fd(), sys::STDOUT_FILENO);
        }
        if let Some(ref file) = *stderr {
            redir(file.as_raw_fd(), sys::STDERR_FILENO);
        }

        let function = self.functions.get(name).cloned().unwrap();
        match function.execute(self, args) {
            Ok(()) => SUCCESS,
            Err(FunctionError::InvalidArgumentCount) => {
                eprintln!("ion: invalid number of function arguments supplied");
                FAILURE
            }
            Err(FunctionError::InvalidArgumentType(expected_type, value)) => {
                eprintln!(
                    "ion: function argument has invalid type: expected {}, found value \'{}\'",
                    expected_type,
                    value
                );
                FAILURE
            }
        }
    }

    fn exec_multi_in(
        &mut self,
        sources: &mut [File],
        piped: bool,
        stdout: &Option<File>,
        stdin: &Option<File>,
    ) -> i32 {
        if let Some(ref file) = *stdin {
            redir(file.as_raw_fd(), sys::STDIN_FILENO)
        }
        if let Some(ref file) = *stdout {
            redir(file.as_raw_fd(), sys::STDOUT_FILENO)
        }

        fn read_and_write<R: io::Read>(src: &mut R, stdout: &mut io::StdoutLock)
            -> io::Result<()> {
            let mut buf = [0; 4096];
            loop {
                let len = src.read(&mut buf)?;
                if len == 0 { return Ok(()) };
                let mut total = 0;
                loop {
                    let wrote = stdout.write(&buf[total..len])?;
                    total += wrote;
                    if total == len { break; }
                }
            }
        };
        let stdout = io::stdout();
        let mut stdout = stdout.lock();
        if piped {
            let stdin = io::stdin();
            let stdin = &mut stdin.lock();
            match read_and_write(stdin, &mut stdout) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!(
                        "ion: error in multiple input redirect process: {:?}",
                        e
                    );
                    return FAILURE;
                }
            }
        }
        for file in sources {
            match read_and_write(file, &mut stdout) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!(
                        "ion: error in multiple input redirect process: {:?}",
                        e
                    );
                    return FAILURE;
                }
            }
        }
        SUCCESS
    }
}

/// This function will panic if called with an empty slice
pub(crate) fn pipe(
    shell: &mut Shell,
    commands: Vec<(RefinedJob, JobKind)>,
    foreground: bool,
) -> i32 {
    fn close(file: &Option<File>) {
        if let &Some(ref file) = file {
            if let Err(e) = sys::close(file.as_raw_fd()) {
                eprintln!("ion: failed to close file '{:?}': {}", file, e);
            }
        }
    }

    let mut previous_status = SUCCESS;
    let mut previous_kind = JobKind::And;
    let mut commands = commands.into_iter();
    loop {
        if let Some((mut parent, mut kind)) = commands.next() {
            // When an `&&` or `||` operator is utilized, execute commands based on the previous
            // status.
            match previous_kind {
                JobKind::And => if previous_status != SUCCESS {
                    if let JobKind::Or = kind {
                        previous_kind = kind
                    }
                    continue;
                },
                JobKind::Or => if previous_status == SUCCESS {
                    if let JobKind::And = kind {
                        previous_kind = kind
                    }
                    continue;
                },
                _ => (),
            }

            match kind {
                JobKind::Pipe(mut mode) => {
                    // We need to remember the commands as they own the file
                    // descriptors that are created by sys::pipe.
                    // We purposfully drop the pipes that are owned by a given
                    // command in `wait` in order to close those pipes, sending
                    // EOF to the next command
                    let mut remember = Vec::new();
                    // A list of the PIDs in the piped command
                    let mut children: Vec<u32> = Vec::new();
                    // The process group by which all of the PIDs belong to.
                    let mut pgid = 0; // 0 means the PGID is not set yet.

                    macro_rules! spawn_proc {
                        ($cmd:expr) => {
                            let short = $cmd.short();
                            match $cmd {
                                RefinedJob::External(ref mut command) => {
                                    match {
                                        command.before_exec(move || {
                                            signals::unblock();
                                            create_process_group(pgid);
                                            Ok(())
                                        }).spawn()
                                    } {
                                        Ok(child) => {
                                            if pgid == 0 {
                                                pgid = child.id();
                                                if foreground && !shell.is_library {
                                                    let _ = sys::tcsetpgrp(0, pgid);
                                                }
                                            }
                                            shell.foreground.push(child.id());
                                            children.push(child.id());
                                        },
                                        Err(e) => {
                                            return if e.kind() == io::ErrorKind::NotFound {
                                                eprintln!("ion: command not found: {}", short);
                                                NO_SUCH_COMMAND
                                            } else {
                                                eprintln!("ion: error spawning process: {}", e);
                                                COULD_NOT_EXEC
                                            }
                                        }
                                    }
                                }
                                RefinedJob::Builtin { ref name,
                                                      ref args,
                                                      ref stdout,
                                                      ref stderr,
                                                      ref stdin, } =>
                                {
                                    match unsafe { sys::fork() } {
                                        Ok(0) => {
                                            signals::unblock();
                                            let _ = sys::reset_signal(sys::SIGINT);
                                            let _ = sys::reset_signal(sys::SIGHUP);
                                            let _ = sys::reset_signal(sys::SIGTERM);
                                            create_process_group(pgid);
                                            let args: Vec<&str> = args
                                                .iter()
                                                .map(|x| x as &str).collect();
                                            let ret = shell.exec_builtin(name,
                                                              &args,
                                                              stdout,
                                                              stderr,
                                                              stdin);
                                            close(stdout);
                                            close(stderr);
                                            close(stdin);
                                            exit(ret)
                                        },
                                        Ok(pid) => {
                                            if pgid == 0 {
                                                pgid = pid;
                                                if foreground && !shell.is_library {
                                                    let _ = sys::tcsetpgrp(0, pgid);
                                                }
                                            }
                                            shell.foreground.push(pid);
                                            children.push(pid);
                                        },
                                        Err(e) => {
                                            eprintln!("ion: failed to fork {}: {}",
                                                      short,
                                                      e);
                                        }
                                    }
                                }
                                RefinedJob::Function { ref name,
                                                      ref args,
                                                      ref stdout,
                                                      ref stderr,
                                                      ref stdin, } =>
                                {
                                    match unsafe { sys::fork() } {
                                        Ok(0) => {
                                            // signals::unblock();
                                            let _ = sys::reset_signal(sys::SIGINT);
                                            let _ = sys::reset_signal(sys::SIGHUP);
                                            let _ = sys::reset_signal(sys::SIGTERM);
                                            create_process_group(pgid);
                                            let args: Vec<&str> = args
                                                .iter()
                                                .map(|x| x as &str).collect();
                                            let ret = shell.exec_function(name,
                                                              &args,
                                                              stdout,
                                                              stderr,
                                                              stdin);
                                            close(stdout);
                                            close(stderr);
                                            close(stdin);
                                            exit(ret)
                                        },
                                        Ok(pid) => {
                                            if pgid == 0 {
                                                pgid = pid;
                                                if foreground && !shell.is_library {
                                                    let _ = sys::tcsetpgrp(0, pgid);
                                                }
                                            }
                                            shell.foreground.push(pid);
                                            children.push(pid);
                                        },
                                        Err(e) => {
                                            eprintln!("ion: failed to fork {}: {}",
                                                      short,
                                                      e);
                                        }
                                    }
                                }
                                RefinedJob::Cat { ref mut sources,
                                                  piped,
                                                  ref stdin,
                                                  ref stdout} => {
                                    match unsafe { sys::fork() } {
                                        Ok(0) => {
                                            let _ = sys::reset_signal(sys::SIGINT);
                                            let _ = sys::reset_signal(sys::SIGHUP);
                                            let _ = sys::reset_signal(sys::SIGTERM);
                                            create_process_group(pgid);
                                            let ret = shell.exec_multi_in(
                                                sources,
                                                piped,
                                                stdin,
                                                stdout,
                                            );
                                            close(stdout);
                                            close(stdin);
                                            exit(ret);
                                        }
                                        Ok(pid) => {
                                            if pgid == 0 {
                                                pgid = pid;
                                                if foreground && !shell.is_library {
                                                    let _ = sys::tcsetpgrp(0, pgid);
                                                }
                                            }
                                            shell.foreground.push(pid);
                                            children.push(pid);
                                        }
                                        Err(e) => eprintln!("ion: failed to fork {}: {}", short, e),
                                    }
                                },
                                RefinedJob::Tee { .. } => {}
                            }
                        };
                    }

                    // TODO: RedirectKind
                    // Append other jobs until all piped jobs are running
                    while let Some((mut child, ckind)) = commands.next() {
                        match sys::pipe2(sys::O_CLOEXEC) {
                            Err(e) => {
                                eprintln!("ion: failed to create pipe: {:?}", e);
                            }
                            Ok((reader, writer)) => {
                                child.stdin(unsafe { File::from_raw_fd(reader) });
                                match mode {
                                    RedirectFrom::Stderr => {
                                        parent.stderr(unsafe { File::from_raw_fd(writer) });
                                    }
                                    RedirectFrom::Stdout => {
                                        parent.stdout(unsafe { File::from_raw_fd(writer) });
                                    }
                                    RedirectFrom::Both => {
                                        let temp = unsafe { File::from_raw_fd(writer) };
                                        match temp.try_clone() {
                                            Err(e) => {
                                                eprintln!(
                                                    "ion: failed to redirect stdout and stderr: {}",
                                                    e
                                                );
                                            }
                                            Ok(duped) => {
                                                parent.stderr(temp);
                                                parent.stdout(duped);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        spawn_proc!(parent);
                        remember.push(parent);
                        if let JobKind::Pipe(m) = ckind {
                            parent = child;
                            mode = m;
                        } else {
                            // We set the kind to the last child kind that was
                            // processed. For example, the pipeline
                            // `foo | bar | baz && zardoz` should have the
                            // previous kind set to `And` after processing the
                            // initial pipeline
                            kind = ckind;
                            spawn_proc!(child);
                            remember.push(child);
                            break;
                        }
                    }
                    previous_kind = kind;
                    previous_status = shell.wait(children, remember);
                    if previous_status == TERMINATED {
                        shell.foreground_send(sys::SIGTERM);
                        return previous_status;
                    }
                }
                _ => {
                    previous_status = shell.exec_job(&mut parent, foreground);
                    previous_kind = kind;
                }
            }
        } else {
            break;
        }
    }
    previous_status
}
