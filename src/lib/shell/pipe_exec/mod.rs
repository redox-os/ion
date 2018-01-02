//! The purpose of the pipeline execution module is to create commands from
//! supplied pieplines, and manage their execution thereof. That includes
//! forking, executing commands, managing process group
//! IDs, watching foreground and background tasks, sending foreground tasks to
//! the background, handling pipeline and conditional operators, and
//! std{in,out,err} redirections.

mod command_not_found;
pub mod foreground;
mod fork;
pub mod job_control;
mod streams;

// TODO: Reintegrate this
use self::command_not_found::command_not_found;
use self::fork::fork_pipe;
use self::job_control::{JobControl, ProcessState};
use self::streams::{duplicate_streams, redir, redirect_streams};
use super::{JobKind, Shell};
use super::flags::*;
use super::flow_control::FunctionError;
use super::job::{RefinedJob, TeeItem};
use super::signals::{self, SignalHandler};
use super::status::*;
use builtins::{self, BuiltinFunction};
use parser::pipelines::{Input, PipeItem, Pipeline, RedirectFrom, Redirection};
use std::fs::{File, OpenOptions};
use std::io::{self, Error, Write};
use std::iter;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::path::Path;
use std::process::{self, exit};
use sys;

type RefinedItem = (RefinedJob, JobKind, Vec<Redirection>, Vec<Input>);

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
    // `infile` currently owns the writer end RawFd. If we just return the reader
    // end and let `infile` go out of scope, it will be closed, sending EOF to
    // the reader!
    Ok(reader)
}

/// This function serves three purposes:
/// 1. If the result is `Some`, then we will fork the pipeline executing into the background.
/// 2. The value stored within `Some` will be that background job's command name.
/// 3. If `set -x` was set, print the command.
fn gen_background_string(pipeline: &Pipeline, print_comm: bool) -> Option<(String, bool)> {
    let last = &pipeline.items[pipeline.items.len() - 1];
    if last.job.kind == JobKind::Background || last.job.kind == JobKind::Disown {
        let command = pipeline.to_string();
        if print_comm {
            eprintln!("> {}", command);
        }
        Some((command, last.job.kind == JobKind::Disown))
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

/// Insert the multiple redirects as pipelines if necessary. Handle both input and output
/// redirection if necessary.
fn do_redirection(piped_commands: Vec<RefinedItem>) -> Option<Vec<(RefinedJob, JobKind)>> {
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
                Input::HereString(ref mut string) => {
                    if !string.ends_with('\n') {
                        string.push('\n');
                    }
                    match unsafe { stdin_of(&string) } {
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
    }

    let need_tee = |outs: &[_], kind| {
        let (mut stdout_count, mut stderr_count) = (0, 0);
        match kind {
            JobKind::Pipe(RedirectFrom::Both) => {
                stdout_count += 1;
                stderr_count += 1;
            }
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
                (Some(tee), None)
            } else {
                (None, Some(tee))
            };
            let tee = RefinedJob::tee(items.0, items.1);
            $new.push((tee, $kind));
        }}
    }

    // Real logic begins here
    let mut new_commands = Vec::new();
    let mut prev_kind = JobKind::And;
    for (mut job, kind, outputs, mut inputs) in piped_commands {
        match (inputs.len(), prev_kind) {
            (0, _) => {}
            (1, JobKind::Pipe(_)) => {
                let sources = vec![get_infile!(inputs[0])?];
                new_commands.push((
                    RefinedJob::cat(sources),
                    JobKind::Pipe(RedirectFrom::Stdout),
                ));
            }
            (1, _) => job.stdin(get_infile!(inputs[0])?),
            _ => {
                let mut sources = Vec::new();
                for mut input in inputs {
                    sources.push(if let Some(f) = get_infile!(input) {
                        f
                    } else {
                        return None;
                    });
                }
                new_commands.push((
                    RefinedJob::cat(sources),
                    JobKind::Pipe(RedirectFrom::Stdout),
                ));
            }
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
                let mut tee_out = TeeItem {
                    sinks:  Vec::new(),
                    source: None,
                };
                let mut tee_err = TeeItem {
                    sinks:  Vec::new(),
                    source: None,
                };
                for output in outputs {
                    match if output.append {
                        OpenOptions::new()
                            .create(true)
                            .write(true)
                            .append(true)
                            .open(&output.file)
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
                                }
                                Err(e) => {
                                    eprintln!(
                                        "ion: failed to redirect both stdout and stderr to file \
                                         '{:?}': {}",
                                        f, e
                                    );
                                    return None;
                                }
                            },
                        },
                        Err(e) => {
                            eprintln!("ion: failed to redirect output into {}: {}", output.file, e);
                            return None;
                        }
                    }
                }
                let tee = RefinedJob::tee(Some(tee_out), Some(tee_err));
                new_commands.push((job, JobKind::Pipe(RedirectFrom::Stdout)));
                new_commands.push((tee, kind));
            }
        }
    }
    Some(new_commands)
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
    fn generate_commands(&self, pipeline: &mut Pipeline) -> Result<Vec<RefinedItem>, i32>;

    /// Waits for all of the children of the assigned pgid to finish executing, returning the
    /// exit status of the last process in the queue.
    fn wait(&mut self, pgid: u32, commands: Vec<RefinedJob>) -> i32;

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
        main: BuiltinFunction,
        args: &[&str],
        stdout: &Option<File>,
        stderr: &Option<File>,
        stdin: &Option<File>,
    ) -> i32;

    fn exec_external(
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
    fn exec_multi_in(
        &mut self,
        sources: &mut [File],
        stdout: &Option<File>,
        stdin: &mut Option<File>,
    ) -> i32;

    /// For tee jobs
    fn exec_multi_out(
        &mut self,
        items: &mut (Option<TeeItem>, Option<TeeItem>),
        stdout: &Option<File>,
        stderr: &Option<File>,
        stdin: &Option<File>,
        kind: JobKind,
    ) -> i32;
}

impl PipelineExecution for Shell {
    fn execute_pipeline(&mut self, pipeline: &mut Pipeline) -> i32 {
        // If the supplied pipeline is a background, a string representing the command
        // and a boolean representing whether it should be disowned is stored here.
        let possible_background_name =
            gen_background_string(&pipeline, self.flags & PRINT_COMMS != 0);
        // Generates commands for execution, differentiating between external and
        // builtin commands.
        let piped_commands = match self.generate_commands(pipeline) {
            Ok(commands) => commands,
            Err(error) => return error,
        };

        // Don't execute commands when the `-n` flag is passed.
        if self.flags & NO_EXEC != 0 {
            return SUCCESS;
        }

        let piped_commands = if let Some(c) = do_redirection(piped_commands) {
            c
        } else {
            return COULD_NOT_EXEC;
        };

        // If the given pipeline is a background task, fork the shell.
        if let Some((command_name, disown)) = possible_background_name {
            fork_pipe(
                self,
                piped_commands,
                command_name,
                if disown {
                    ProcessState::Empty
                } else {
                    ProcessState::Running
                },
            )
        } else {
            // While active, the SIGTTOU signal will be ignored.
            let _sig_ignore = SignalHandler::new();
            let foreground = !self.is_background_shell;
            // Execute each command in the pipeline, giving each command the foreground.
            let exit_status = pipe(self, piped_commands, foreground);
            // Set the shell as the foreground process again to regain the TTY.
            if foreground && !self.is_library {
                let _ = sys::tcsetpgrp(0, process::id());
            }
            exit_status
        }
    }

    fn generate_commands(&self, pipeline: &mut Pipeline) -> Result<Vec<RefinedItem>, i32> {
        let mut results = Vec::new();
        for item in pipeline.items.drain(..) {
            let PipeItem {
                mut job,
                outputs,
                inputs,
            } = item;
            let refined = {
                if is_implicit_cd(&job.args[0]) {
                    RefinedJob::builtin(
                        builtins::builtin_cd,
                        iter::once("cd".into()).chain(job.args.drain()).collect(),
                    )
                } else if self.functions.contains_key(job.args[0].as_str()) {
                    RefinedJob::function(job.args[0].clone().into(), job.args.drain().collect())
                } else if let Some(builtin) = job.builtin {
                    RefinedJob::builtin(builtin, job.args.drain().collect())
                } else {
                    RefinedJob::external(job.args[0].clone().into(), job.args.drain().collect())
                }
            };
            results.push((refined, job.kind, outputs, inputs));
        }

        Ok(results)
    }

    fn wait(&mut self, pgid: u32, commands: Vec<RefinedJob>) -> i32 {
        // TODO: Find a way to only do this when absolutely necessary.
        let as_string = commands
            .iter()
            .map(RefinedJob::long)
            .collect::<Vec<String>>()
            .join(" | ");

        // Watch the foreground group, dropping all commands that exit as they exit.
        self.watch_foreground(-(pgid as i32), &as_string)
    }

    fn exec_job(&mut self, job: &mut RefinedJob, _foreground: bool) -> i32 {
        let short = job.short();
        let long = job.long();
        match *job {
            RefinedJob::External {
                ref name,
                ref args,
                ref stdin,
                ref stdout,
                ref stderr,
            } => {
                let args: Vec<&str> = args.iter().skip(1).map(|x| x as &str).collect();
                return self.exec_external(&name, &args, stdin, stdout, stderr);
            }
            RefinedJob::Builtin {
                main,
                ref args,
                ref stdin,
                ref stdout,
                ref stderr,
            } => {
                if let Ok((stdin_bk, stdout_bk, stderr_bk)) = duplicate_streams() {
                    let args: Vec<&str> = args.iter().map(|x| x as &str).collect();
                    let code = self.exec_builtin(main, &args, stdout, stderr, stdin);
                    redirect_streams(stdin_bk, stdout_bk, stderr_bk);
                    return code;
                }
                eprintln!(
                    "ion: failed to `dup` STDOUT, STDIN, or STDERR: not running '{}'",
                    long
                );
                COULD_NOT_EXEC
            }
            RefinedJob::Function {
                ref name,
                ref args,
                ref stdin,
                ref stdout,
                ref stderr,
            } => {
                if let Ok((stdin_bk, stdout_bk, stderr_bk)) = duplicate_streams() {
                    let args: Vec<&str> = args.iter().map(|x| x as &str).collect();
                    let code = self.exec_function(name, &args, stdout, stderr, stdin);
                    redirect_streams(stdin_bk, stdout_bk, stderr_bk);
                    return code;
                }
                eprintln!(
                    "ion: failed to `dup` STDOUT, STDIN, or STDERR: not running '{}'",
                    long
                );
                COULD_NOT_EXEC
            }
            _ => panic!("exec job should not be able to be called on Cat or Tee jobs"),
        }
    }

    fn exec_builtin(
        &mut self,
        main: BuiltinFunction,
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

        main(args, self)
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
                    expected_type, value
                );
                FAILURE
            }
        }
    }

    fn exec_multi_in(
        &mut self,
        sources: &mut [File],
        stdout: &Option<File>,
        stdin: &mut Option<File>,
    ) -> i32 {
        if let Some(ref file) = *stdin {
            redir(file.as_raw_fd(), sys::STDIN_FILENO)
        }
        if let Some(ref file) = *stdout {
            redir(file.as_raw_fd(), sys::STDOUT_FILENO)
        }

        fn read_and_write<R: io::Read>(src: &mut R, stdout: &mut io::StdoutLock) -> io::Result<()> {
            let mut buf = [0; 4096];
            loop {
                let len = src.read(&mut buf)?;
                if len == 0 {
                    return Ok(());
                };
                let mut total = 0;
                loop {
                    let wrote = stdout.write(&buf[total..len])?;
                    total += wrote;
                    if total == len {
                        break;
                    }
                }
            }
        };
        let stdout = io::stdout();
        let mut stdout = stdout.lock();
        for file in stdin.iter_mut().chain(sources) {
            if let Err(why) = read_and_write(file, &mut stdout) {
                eprintln!("ion: error in multiple input redirect process: {:?}", why);
                return FAILURE;
            }
        }
        SUCCESS
    }

    fn exec_multi_out(
        &mut self,
        items: &mut (Option<TeeItem>, Option<TeeItem>),
        stdout: &Option<File>,
        stderr: &Option<File>,
        stdin: &Option<File>,
        kind: JobKind,
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
        let res = match items {
            &mut (None, None) => panic!("There must be at least one TeeItem, this is a bug"),
            &mut (Some(ref mut tee_out), None) => match kind {
                JobKind::Pipe(RedirectFrom::Stderr) => tee_out.write_to_all(None),
                JobKind::Pipe(_) => tee_out.write_to_all(Some(RedirectFrom::Stdout)),
                _ => tee_out.write_to_all(None),
            },
            &mut (None, Some(ref mut tee_err)) => match kind {
                JobKind::Pipe(RedirectFrom::Stdout) => tee_err.write_to_all(None),
                JobKind::Pipe(_) => tee_err.write_to_all(Some(RedirectFrom::Stderr)),
                _ => tee_err.write_to_all(None),
            },
            &mut (Some(ref mut tee_out), Some(ref mut tee_err)) => {
                // TODO Make it work with pipes
                if let Err(e) = tee_out.write_to_all(None) {
                    Err(e)
                } else {
                    tee_err.write_to_all(None)
                }
            }
        };
        if let Err(e) = res {
            eprintln!("ion: error in multiple output redirection process: {:?}", e);
            FAILURE
        } else {
            SUCCESS
        }
    }

    fn exec_external(
        &mut self,
        name: &str,
        args: &[&str],
        stdin: &Option<File>,
        stdout: &Option<File>,
        stderr: &Option<File>,
    ) -> i32 {
        let result = sys::fork_and_exec(
            name,
            &args,
            if let Some(ref f) = *stdin {
                Some(f.as_raw_fd())
            } else {
                None
            },
            if let Some(ref f) = *stdout {
                Some(f.as_raw_fd())
            } else {
                None
            },
            if let Some(ref f) = *stderr {
                Some(f.as_raw_fd())
            } else {
                None
            },
            false,
            || prepare_child(false),
        );

        match result {
            Ok(pid) => self.watch_foreground(pid as i32, ""),
            Err(ref err) if err.kind() == io::ErrorKind::NotFound => {
                if !command_not_found(self, &name) {
                    eprintln!("ion: command not found: {}", name);
                }
                NO_SUCH_COMMAND
            }
            Err(ref err) => {
                eprintln!("ion: command exec error: {}", err);
                FAILURE
            }
        }
    }
}

/// Executes a piped job `job1 | job2 | job3`
///
/// This function will panic if called with an empty slice
pub(crate) fn pipe(
    shell: &mut Shell,
    commands: Vec<(RefinedJob, JobKind)>,
    foreground: bool,
) -> i32 {
    let mut previous_status = SUCCESS;
    let mut previous_kind = JobKind::And;
    let mut commands = commands.into_iter().peekable();
    let mut possible_external_stdio_pipes: Option<Vec<File>> = None;

    loop {
        if let Some((mut parent, mut kind)) = commands.next() {
            // When an `&&` or `||` operator is utilized, execute commands based on the
            // previous status.
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
                    let mut remember = Vec::new();

                    let mut pgid = 0;
                    let mut last_pid = 0;
                    let mut current_pid = 0;

                    // When set to true, this command will be SIGSTOP'd  before it executes.
                    let mut child_blocked;

                    // Append jobs until all piped jobs are running
                    while let Some((mut child, ckind)) = commands.next() {
                        // If parent is a RefindJob::External, then we need to keep track of the
                        // output pipes, so we can properly close them after the job has been
                        // spawned.
                        let is_external = if let RefinedJob::External { .. } = parent {
                            true
                        } else {
                            false
                        };

                        // If we need to tee both stdout and stderr, we directly connect pipes to
                        // the relevant sources in both of them.
                        if let RefinedJob::Tee {
                            items: (Some(ref mut tee_out), Some(ref mut tee_err)),
                            ..
                        } = child
                        {
                            match sys::pipe2(sys::O_CLOEXEC) {
                                Err(e) => eprintln!("ion: failed to create pipe: {:?}", e),
                                Ok((out_reader, out_writer)) => {
                                    (*tee_out).source =
                                        Some(unsafe { File::from_raw_fd(out_reader) });
                                    parent.stdout(unsafe { File::from_raw_fd(out_writer) });
                                    if is_external {
                                        possible_external_stdio_pipes
                                            .get_or_insert(vec![])
                                            .push(unsafe { File::from_raw_fd(out_writer) });
                                    }
                                }
                            }
                            match sys::pipe2(sys::O_CLOEXEC) {
                                Err(e) => eprintln!("ion: failed to create pipe: {:?}", e),
                                Ok((err_reader, err_writer)) => {
                                    (*tee_err).source =
                                        Some(unsafe { File::from_raw_fd(err_reader) });
                                    parent.stderr(unsafe { File::from_raw_fd(err_writer) });
                                    if is_external {
                                        possible_external_stdio_pipes
                                            .get_or_insert(vec![])
                                            .push(unsafe { File::from_raw_fd(err_writer) });
                                    }
                                }
                            }
                        } else {
                            match sys::pipe2(sys::O_CLOEXEC) {
                                Err(e) => {
                                    eprintln!("ion: failed to create pipe: {:?}", e);
                                }
                                Ok((reader, writer)) => {
                                    if is_external {
                                        possible_external_stdio_pipes
                                            .get_or_insert(vec![])
                                            .push(unsafe { File::from_raw_fd(writer) });
                                    }
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
                                                        "ion: failed to redirect stdout and \
                                                         stderr: {}",
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
                        }

                        child_blocked = match ckind {
                            JobKind::Pipe(_) | JobKind::Last => true,
                            _ => false,
                        };

                        match spawn_proc(
                            shell,
                            parent,
                            kind,
                            child_blocked,
                            &mut last_pid,
                            &mut current_pid,
                        ) {
                            SUCCESS => (),
                            error_code => return error_code,
                        }

                        // remember.push(parent);
                        possible_external_stdio_pipes = None;

                        if set_process_group(&mut pgid, current_pid) && foreground
                            && !shell.is_library
                        {
                            let _ = sys::tcsetpgrp(0, pgid);
                        }

                        resume_prior_process(&mut last_pid, current_pid, child_blocked);

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

                            child_blocked = match commands.peek() {
                                Some(&(_, JobKind::Pipe(_))) => true,
                                Some(&(_, JobKind::Last)) => true,
                                _ => false,
                            };

                            match spawn_proc(
                                shell,
                                child,
                                kind,
                                child_blocked,
                                &mut last_pid,
                                &mut current_pid,
                            ) {
                                SUCCESS => (),
                                error_code => return error_code,
                            }

                            set_process_group(&mut pgid, current_pid) && foreground
                                && !shell.is_library;

                            // remember.push(child);
                            resume_prior_process(&mut last_pid, current_pid, child_blocked);

                            break;
                        }
                    }
                    previous_kind = kind;
                    previous_status = shell.wait(pgid, remember);
                    if previous_status == TERMINATED {
                        if let Err(why) = sys::killpg(pgid, sys::SIGTERM) {
                            eprintln!("ion: failed to terminate foreground jobs: {}", why);
                        }
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

fn spawn_proc(
    shell: &mut Shell,
    mut cmd: RefinedJob,
    kind: JobKind,
    child_blocked: bool,
    last_pid: &mut u32,
    current_pid: &mut u32,
) -> i32 {
    let short = cmd.short();
    match cmd {
        RefinedJob::External {
            ref name,
            ref args,
            ref stdout,
            ref stderr,
            ref stdin,
        } => {
            let args: Vec<&str> = args.iter().skip(1).map(|x| x as &str).collect();
            let result = sys::fork_and_exec(
                name,
                &args,
                if let Some(ref f) = *stdin {
                    Some(f.as_raw_fd())
                } else {
                    None
                },
                if let Some(ref f) = *stdout {
                    Some(f.as_raw_fd())
                } else {
                    None
                },
                if let Some(ref f) = *stderr {
                    Some(f.as_raw_fd())
                } else {
                    None
                },
                false,
                || prepare_child(child_blocked),
            );

            match result {
                Ok(pid) => {
                    *last_pid = *current_pid;
                    *current_pid = pid;
                }
                Err(ref err) if err.kind() == io::ErrorKind::NotFound => {
                    if !command_not_found(shell, &name) {
                        eprintln!("ion: command not found: {}", name);
                    }
                }
                Err(ref err) => {
                    eprintln!("ion: command exec error: {}", err);
                }
            }
        }
        RefinedJob::Builtin {
            main,
            ref args,
            ref stdout,
            ref stderr,
            ref stdin,
        } => {
            let args: Vec<&str> = args.iter().map(|x| x as &str).collect();
            match unsafe { sys::fork() } {
                Ok(0) => {
                    prepare_child(child_blocked);
                    let ret = shell.exec_builtin(main, &args, stdout, stderr, stdin);
                    close(stdout);
                    close(stderr);
                    close(stdin);
                    exit(ret)
                }
                Ok(pid) => {
                    close(stdout);
                    close(stderr);
                    *last_pid = *current_pid;
                    *current_pid = pid;
                }
                Err(e) => {
                    eprintln!("ion: failed to fork {}: {}", short, e);
                }
            }
        }
        RefinedJob::Function {
            ref name,
            ref args,
            ref stdout,
            ref stderr,
            ref stdin,
        } => {
            let args: Vec<&str> = args.iter().map(|x| x as &str).collect();
            match unsafe { sys::fork() } {
                Ok(0) => {
                    prepare_child(child_blocked);
                    let ret = shell.exec_function(name, &args, stdout, stderr, stdin);
                    close(stdout);
                    close(stderr);
                    close(stdin);
                    exit(ret)
                }
                Ok(pid) => {
                    close(stdout);
                    close(stderr);
                    *last_pid = *current_pid;
                    *current_pid = pid;
                }
                Err(e) => {
                    eprintln!("ion: failed to fork {}: {}", short, e);
                }
            }
        }
        RefinedJob::Cat {
            ref mut sources,
            ref stdout,
            ref mut stdin,
        } => match unsafe { sys::fork() } {
            Ok(0) => {
                prepare_child(child_blocked);

                let ret = shell.exec_multi_in(sources, stdout, stdin);
                close(stdout);
                close(stdin);
                exit(ret);
            }
            Ok(pid) => {
                close(stdout);
                *last_pid = *current_pid;
                *current_pid = pid;
            }
            Err(e) => eprintln!("ion: failed to fork {}: {}", short, e),
        },
        RefinedJob::Tee {
            ref mut items,
            ref stdout,
            ref stderr,
            ref stdin,
        } => match unsafe { sys::fork() } {
            Ok(0) => {
                prepare_child(child_blocked);

                let ret = shell.exec_multi_out(items, stdout, stderr, stdin, kind);
                close(stdout);
                close(stderr);
                close(stdin);
                exit(ret);
            }
            Ok(pid) => {
                close(stdout);
                close(stderr);
                *last_pid = *current_pid;
                *current_pid = pid;
            }
            Err(e) => eprintln!("ion: failed to fork {}: {}", short, e),
        },
    }
    SUCCESS
}

// TODO: Don't require this.
fn close(file: &Option<File>) {
    if let &Some(ref file) = file {
        if let Err(e) = sys::close(file.as_raw_fd()) {
            eprintln!("ion: failed to close file '{:?}': {}", file, e);
        }
    }
}

fn prepare_child(child_blocked: bool) {
    signals::unblock();
    let _ = sys::reset_signal(sys::SIGINT);
    let _ = sys::reset_signal(sys::SIGHUP);
    let _ = sys::reset_signal(sys::SIGTERM);

    if child_blocked {
        let _ = sys::kill(process::id(), sys::SIGSTOP);
    } else {
    }
}

fn resume_prior_process(last_pid: &mut u32, current_pid: u32, child_blocked: bool) {
    if child_blocked {
        // Ensure that the process is stopped before continuing.
        if let Err(why) = sys::wait_for_interrupt(current_pid) {
            eprintln!("ion: error waiting for sigstop: {}", why);
        }
    }

    if *last_pid != 0 {
        let _ = sys::kill(*last_pid, sys::SIGCONT);
    }

    *last_pid = current_pid;
}

fn set_process_group(pgid: &mut u32, pid: u32) -> bool {
    let pgid_set = *pgid == 0;
    if pgid_set {
        *pgid = pid;
    }
    let _ = sys::setpgid(pid, *pgid);
    pgid_set
}
