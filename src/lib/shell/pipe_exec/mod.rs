//! The purpose of the pipeline execution module is to create commands from
//! supplied pieplines, and manage their execution thereof. That includes
//! forking, executing commands, managing process group
//! IDs, watching foreground and background tasks, sending foreground tasks to
//! the background, handling pipeline and conditional operators, and
//! std{in,out,err} redirections.

pub mod foreground;
mod fork;
pub mod job_control;
mod pipes;
pub mod streams;

use self::{
    fork::fork_pipe,
    job_control::{JobControl, ProcessState},
    pipes::TeePipe,
    streams::{duplicate_streams, redir, redirect_streams},
};
use super::{
    flags::*,
    flow_control::{Function, FunctionError},
    fork_function::command_not_found,
    job::{JobVariant, RefinedJob, TeeItem},
    signals::{self, SignalHandler},
    status::*,
    JobKind, Shell,
};
use crate::{
    builtins::{self, BuiltinFunction},
    parser::pipelines::{Input, PipeItem, Pipeline, RedirectFrom, Redirection},
    sys,
};
use small;
use smallvec::SmallVec;
use std::{
    fs::{File, OpenOptions},
    io::{self, Error, Write},
    iter,
    os::unix::io::{AsRawFd, FromRawFd, RawFd},
    path::Path,
    process::{self, exit},
};

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
fn do_redirection(
    piped_commands: SmallVec<[RefinedItem; 16]>,
) -> Option<SmallVec<[(RefinedJob, JobKind); 16]>> {
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
                    OpenOptions::new()
                        .create(true)
                        .write(true)
                        .append(true)
                        .open(output.file.as_str())
                } else {
                    File::create(output.file.as_str())
                } {
                    Ok(f) => match output.from {
                        RedirectFrom::Stderr => $job.stderr(f),
                        RedirectFrom::Stdout => $job.stdout(f),
                        RedirectFrom::Both => match f.try_clone() {
                            Ok(f_copy) => {
                                $job.stdout(f);
                                $job.stderr(f_copy);
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
        };
    }

    macro_rules! set_one_tee {
        ($new:ident, $outputs:ident, $job:ident, $kind:ident, $teed:ident, $other:ident) => {{
            let mut tee = TeeItem { sinks: Vec::new(), source: None };
            for output in $outputs {
                match if output.append {
                    OpenOptions::new()
                        .create(true)
                        .write(true)
                        .append(true)
                        .open(output.file.as_str())
                } else {
                    File::create(output.file.as_str())
                } {
                    Ok(f) => match output.from {
                        RedirectFrom::$teed => tee.sinks.push(f),
                        RedirectFrom::$other => {
                            if RedirectFrom::Stdout == RedirectFrom::$teed {
                                $job.stderr(f);
                            } else {
                                $job.stdout(f);
                            }
                        }
                        RedirectFrom::Both => match f.try_clone() {
                            Ok(f_copy) => {
                                if RedirectFrom::Stdout == RedirectFrom::$teed {
                                    $job.stderr(f);
                                } else {
                                    $job.stdout(f);
                                }
                                tee.sinks.push(f_copy);
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
            $new.push(($job, JobKind::Pipe(RedirectFrom::$teed)));
            let items = if RedirectFrom::Stdout == RedirectFrom::$teed {
                (Some(tee), None)
            } else {
                (None, Some(tee))
            };
            let tee = RefinedJob::tee(items.0, items.1);
            $new.push((tee, $kind));
        }};
    }

    // Real logic begins here
    let mut new_commands = SmallVec::new();
    let mut prev_kind = JobKind::Last;
    for (mut job, kind, outputs, mut inputs) in piped_commands {
        match (inputs.len(), prev_kind) {
            (0, _) => {}
            (1, JobKind::Pipe(_)) => {
                let sources = vec![inputs[0].get_infile()?];
                new_commands.push((RefinedJob::cat(sources), JobKind::Pipe(RedirectFrom::Stdout)));
            }
            (1, _) => job.stdin(inputs[0].get_infile()?),
            _ => {
                let mut sources = Vec::new();
                for input in &mut inputs {
                    sources.push(if let Some(f) = input.get_infile() {
                        f
                    } else {
                        return None;
                    });
                }
                new_commands.push((RefinedJob::cat(sources), JobKind::Pipe(RedirectFrom::Stdout)));
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
                let mut tee_out = TeeItem { sinks: Vec::new(), source: None };
                let mut tee_err = TeeItem { sinks: Vec::new(), source: None };
                for output in outputs {
                    match if output.append {
                        OpenOptions::new()
                            .create(true)
                            .write(true)
                            .append(true)
                            .open(output.file.as_str())
                    } else {
                        File::create(output.file.as_str())
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
    /// continue to watch the job in the background, printing notifications on status changes
    /// of that job over time.
    fn execute_pipeline(&mut self, pipeline: &mut Pipeline) -> i32;

    /// Generates a vector of commands from a given `Pipeline`.
    ///
    /// Each generated command will either be a builtin or external command, and will be
    /// associated will be marked as an `&&`, `||`, `|`, or final job.
    fn generate_commands(
        &self,
        pipeline: &mut Pipeline,
    ) -> Result<SmallVec<[RefinedItem; 16]>, i32>;

    /// Waits for all of the children of the assigned pgid to finish executing, returning the
    /// exit status of the last process in the queue.
    fn wait(&mut self, pgid: u32, commands: SmallVec<[RefinedJob; 16]>) -> i32;

    /// Executes a `RefinedJob` that was created in the `generate_commands` method.
    ///
    /// The aforementioned `RefinedJob` may be either a builtin or external command.
    /// The purpose of this function is therefore to execute both types accordingly.
    fn exec_job(&mut self, job: &mut RefinedJob, foreground: bool) -> i32;

    /// Execute a builtin in the current process.
    /// # Args
    /// * `shell`: A `Shell` that forwards relevant information to the builtin
    /// * `name`: Name of the builtin to execute.
    /// * `stdin`, `stdout`, `stderr`: File descriptors that will replace the respective standard
    ///   streams if they are not `None`
    /// # Preconditions
    /// * `shell.builtins.contains_key(name)`; otherwise this function will panic
    fn exec_builtin(
        &mut self,
        main: BuiltinFunction,
        args: &[small::String],
        stdout: &Option<File>,
        stderr: &Option<File>,
        stdin: &Option<File>,
    ) -> i32;

    fn exec_external<'a, S: AsRef<str>>(
        &mut self,
        name: &'a str,
        args: &'a [S],
        stdout: &Option<File>,
        stderr: &Option<File>,
        stdin: &Option<File>,
    ) -> i32;

    fn exec_function<S: AsRef<str>>(
        &mut self,
        name: &str,
        args: &[S],
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
    fn exec_external<'a, S: AsRef<str>>(
        &mut self,
        name: &'a str,
        args: &'a [S],
        stdin: &Option<File>,
        stdout: &Option<File>,
        stderr: &Option<File>,
    ) -> i32 {
        let result = sys::fork_and_exec(
            name,
            args,
            if let Some(ref f) = *stdin { Some(f.as_raw_fd()) } else { None },
            if let Some(ref f) = *stdout { Some(f.as_raw_fd()) } else { None },
            if let Some(ref f) = *stderr { Some(f.as_raw_fd()) } else { None },
            false,
            || prepare_child(true, 0),
        );

        match result {
            Ok(pid) => {
                let _ = sys::setpgid(pid, pid);
                let _ = sys::tcsetpgrp(0, pid);
                let _ = wait_for_interrupt(pid);
                let _ = sys::kill(pid, sys::SIGCONT);
                self.watch_foreground(-(pid as i32), "")
            }
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
        let res = match *items {
            (None, None) => panic!("There must be at least one TeeItem, this is a bug"),
            (Some(ref mut tee_out), None) => match kind {
                JobKind::Pipe(RedirectFrom::Stderr) => tee_out.write_to_all(None),
                JobKind::Pipe(_) => tee_out.write_to_all(Some(RedirectFrom::Stdout)),
                _ => tee_out.write_to_all(None),
            },
            (None, Some(ref mut tee_err)) => match kind {
                JobKind::Pipe(RedirectFrom::Stdout) => tee_err.write_to_all(None),
                JobKind::Pipe(_) => tee_err.write_to_all(Some(RedirectFrom::Stderr)),
                _ => tee_err.write_to_all(None),
            },
            (Some(ref mut tee_out), Some(ref mut tee_err)) => {
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

        let stdout = io::stdout();
        let mut stdout = stdout.lock();
        for file in stdin.iter_mut().chain(sources) {
            if let Err(why) = std::io::copy(file, &mut stdout) {
                eprintln!("ion: error in multiple input redirect process: {:?}", why);
                return FAILURE;
            }
        }
        SUCCESS
    }

    fn exec_function<S: AsRef<str>>(
        &mut self,
        name: &str,
        args: &[S],
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

        let function = self.variables.get::<Function>(name).unwrap();
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

    fn exec_builtin(
        &mut self,
        main: BuiltinFunction,
        args: &[small::String],
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

    fn exec_job(&mut self, job: &mut RefinedJob, _foreground: bool) -> i32 {
        // Duplicate file descriptors, execute command, and redirect back.
        if let Ok((stdin_bk, stdout_bk, stderr_bk)) = duplicate_streams() {
            let code = job.exec(self);
            redirect_streams(&stdin_bk, &stdout_bk, &stderr_bk);
            code
        } else {
            eprintln!(
                "ion: failed to `dup` STDOUT, STDIN, or STDERR: not running '{}'",
                job.long()
            );

            COULD_NOT_EXEC
        }
    }

    fn wait(&mut self, pgid: u32, commands: SmallVec<[RefinedJob; 16]>) -> i32 {
        crate::IonPool::string(|as_string| {
            if !commands.is_empty() {
                let mut iter = commands.iter().map(RefinedJob::long);
                if let Some(str) = iter.next() {
                    as_string.push_str(&str);
                    iter.for_each(|str| {
                        as_string.push_str(" | ");
                        as_string.push_str(&str);
                    })
                }
            }

            // Watch the foreground group, dropping all commands that exit as they exit.
            self.watch_foreground(-(pgid as i32), &as_string)
        })
    }

    fn generate_commands(
        &self,
        pipeline: &mut Pipeline,
    ) -> Result<SmallVec<[RefinedItem; 16]>, i32> {
        let mut results: SmallVec<[RefinedItem; 16]> = SmallVec::new();
        for item in pipeline.items.drain(..) {
            let PipeItem { mut job, outputs, inputs } = item;
            let refined = {
                if is_implicit_cd(&job.args[0]) {
                    RefinedJob::builtin(
                        builtins::builtin_cd,
                        iter::once("cd".into()).chain(job.args.drain()).collect(),
                    )
                } else if self.variables.get::<Function>(job.args[0].as_str()).is_some() {
                    RefinedJob::function(job.args[0].clone(), job.args.drain().collect())
                } else if let Some(builtin) = job.builtin {
                    RefinedJob::builtin(builtin, job.args.drain().collect())
                } else {
                    RefinedJob::external(job.args[0].clone(), job.args.drain().collect())
                }
            };
            results.push((refined, job.kind, outputs, inputs));
        }

        Ok(results)
    }

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

        let piped_commands = match do_redirection(piped_commands) {
            Some(c) => c,
            None => return COULD_NOT_EXEC,
        };

        // If the given pipeline is a background task, fork the shell.
        match possible_background_name {
            Some((command_name, disown)) => fork_pipe(
                self,
                piped_commands,
                command_name,
                if disown { ProcessState::Empty } else { ProcessState::Running },
            ),
            None => {
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
    }
}

/// Executes a piped job `job1 | job2 | job3`
///
/// This function will panic if called with an empty slice
pub(crate) fn pipe(
    shell: &mut Shell,
    commands: SmallVec<[(RefinedJob, JobKind); 16]>,
    foreground: bool,
) -> i32 {
    let mut previous_status = SUCCESS;
    let mut commands = commands.into_iter().peekable();
    let mut ext_stdio_pipes: Option<Vec<File>> = None;

    while let Some((mut parent, mut kind)) = commands.next() {
        match kind {
            JobKind::Pipe(mut mode) => {
                // We need to remember the commands as they own the file
                // descriptors that are created by sys::pipe.
                let remember: SmallVec<[RefinedJob; 16]> = SmallVec::new();
                let mut block_child = true;
                let (mut pgid, mut last_pid, mut current_pid) = (0, 0, 0);

                // Append jobs until all piped jobs are running
                while let Some((mut child, ckind)) = commands.next() {
                    // If parent is a RefindJob::External, then we need to keep track of the
                    // output pipes, so we can properly close them after the job has been
                    // spawned.
                    let is_external =
                        if let JobVariant::External { .. } = parent.var { true } else { false };

                    // TODO: Refactor this part
                    // If we need to tee both stdout and stderr, we directly connect pipes to
                    // the relevant sources in both of them.
                    if let JobVariant::Tee {
                        items: (Some(ref mut tee_out), Some(ref mut tee_err)),
                        ..
                    } = child.var
                    {
                        TeePipe::new(&mut parent, &mut ext_stdio_pipes, is_external)
                            .connect(tee_out, tee_err);
                    } else {
                        // Pipe the previous command's stdin to this commands stdout/stderr.
                        match sys::pipe2(sys::O_CLOEXEC) {
                            Err(e) => pipe_fail(&e),
                            Ok((reader, writer)) => {
                                if is_external {
                                    append_external_stdio_pipe(&mut ext_stdio_pipes, writer);
                                }
                                child.stdin(unsafe { File::from_raw_fd(reader) });
                                let writer = unsafe { File::from_raw_fd(writer) };
                                match mode {
                                    RedirectFrom::Stderr => parent.stderr(writer),
                                    RedirectFrom::Stdout => parent.stdout(writer),
                                    RedirectFrom::Both => match writer.try_clone() {
                                        Err(e) => {
                                            eprintln!(
                                                "ion: failed to redirect stdout and stderr: {}",
                                                e
                                            );
                                        }
                                        Ok(duped) => {
                                            parent.stderr(writer);
                                            parent.stdout(duped);
                                        }
                                    },
                                }
                            }
                        }
                    }

                    match spawn_proc(
                        shell,
                        parent,
                        kind,
                        block_child,
                        &mut last_pid,
                        &mut current_pid,
                        pgid,
                    ) {
                        SUCCESS => (),
                        error_code => return error_code,
                    }

                    ext_stdio_pipes = None;

                    if set_process_group(&mut pgid, current_pid) && foreground && !shell.is_library
                    {
                        let _ = sys::tcsetpgrp(0, pgid);
                    }

                    resume_prior_process(&mut last_pid, current_pid);

                    if let JobKind::Pipe(m) = ckind {
                        parent = child;
                        mode = m;
                    } else {
                        kind = ckind;
                        block_child = false;
                        match spawn_proc(
                            shell,
                            child,
                            kind,
                            block_child,
                            &mut last_pid,
                            &mut current_pid,
                            pgid,
                        ) {
                            SUCCESS => (),
                            error_code => return error_code,
                        }

                        resume_prior_process(&mut last_pid, current_pid);
                        break;
                    }
                }

                set_process_group(&mut pgid, current_pid);

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
            }
        }
    }

    let _ = io::stdout().flush();
    let _ = io::stderr().flush();

    previous_status
}

fn spawn_proc(
    shell: &mut Shell,
    mut cmd: RefinedJob,
    kind: JobKind,
    block_child: bool,
    last_pid: &mut u32,
    current_pid: &mut u32,
    pgid: u32,
) -> i32 {
    let stdin = &mut cmd.stdin;
    let stdout = &mut cmd.stdout;
    let stderr = &mut cmd.stderr;
    match cmd.var {
        JobVariant::External { ref mut name, ref mut args } => {
            let args: Vec<&str> = args.iter().skip(1).map(|x| x as &str).collect();
            let mut result = sys::fork_and_exec(
                name,
                &args,
                stdin.as_mut().map(|f| f.as_raw_fd()),
                stdout.as_mut().map(|f| f.as_raw_fd()),
                stderr.as_mut().map(|f| f.as_raw_fd()),
                false,
                || prepare_child(block_child, pgid),
            );

            match result {
                Ok(pid) => {
                    *last_pid = *current_pid;
                    *current_pid = pid;
                }
                Err(ref mut err) if err.kind() == io::ErrorKind::NotFound => {
                    if !command_not_found(shell, &name) {
                        eprintln!("ion: command not found: {}", name);
                    }
                }
                Err(ref mut err) => {
                    eprintln!("ion: command exec error: {}", err);
                }
            }
        }
        JobVariant::Builtin { main, ref mut args } => {
            fork_exec_internal(
                stdout,
                stderr,
                stdin,
                block_child,
                last_pid,
                current_pid,
                pgid,
                |stdout, stderr, stdin| shell.exec_builtin(main, args, stdout, stderr, stdin),
            );
        }
        JobVariant::Function { ref mut name, ref mut args } => {
            fork_exec_internal(
                stdout,
                stderr,
                stdin,
                block_child,
                last_pid,
                current_pid,
                pgid,
                |stdout, stderr, stdin| shell.exec_function(name, &args, stdout, stderr, stdin),
            );
        }
        JobVariant::Cat { ref mut sources } => {
            fork_exec_internal(
                stdout,
                &mut None,
                stdin,
                block_child,
                last_pid,
                current_pid,
                pgid,
                |stdout, _, stdin| shell.exec_multi_in(sources, stdout, stdin),
            );
        }
        JobVariant::Tee { ref mut items } => {
            fork_exec_internal(
                stdout,
                stderr,
                stdin,
                block_child,
                last_pid,
                current_pid,
                pgid,
                |stdout, stderr, stdin| shell.exec_multi_out(items, stdout, stderr, stdin, kind),
            );
        }
    }
    SUCCESS
}

// TODO: Integrate this better within the RefinedJob type.
fn fork_exec_internal<F>(
    stdout: &mut Option<File>,
    stderr: &mut Option<File>,
    stdin: &mut Option<File>,
    block_child: bool,
    last_pid: &mut u32,
    current_pid: &mut u32,
    pgid: u32,
    mut exec_action: F,
) where
    F: FnMut(&mut Option<File>, &mut Option<File>, &mut Option<File>) -> i32,
{
    match unsafe { sys::fork() } {
        Ok(0) => {
            prepare_child(block_child, pgid);

            let exit_status = exec_action(stdout, stderr, stdin);
            stdout.take();
            stderr.take();
            stdin.take();
            exit(exit_status)
        }
        Ok(pid) => {
            stdin.take();
            stdout.take();
            stderr.take();
            *last_pid = *current_pid;
            *current_pid = pid;
        }
        Err(e) => pipe_fail(&e),
    }
}

fn prepare_child(block_child: bool, pgid: u32) {
    signals::unblock();
    let _ = sys::reset_signal(sys::SIGINT);
    let _ = sys::reset_signal(sys::SIGHUP);
    let _ = sys::reset_signal(sys::SIGTERM);

    if block_child {
        let _ = sys::kill(process::id(), sys::SIGSTOP);
    } else {
        let _ = sys::setpgid(process::id(), pgid);
    }
}

fn resume_prior_process(last_pid: &mut u32, current_pid: u32) {
    if *last_pid != 0 {
        // Ensure that the process is stopped before continuing.
        if let Err(why) = wait_for_interrupt(*last_pid) {
            eprintln!("ion: error waiting for sigstop: {}", why);
        }
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

pub fn wait_for_interrupt(pid: u32) -> io::Result<()> {
    loop {
        let mut status = 0;
        match sys::waitpid(pid as i32, &mut status, sys::WUNTRACED) {
            Ok(_) => break Ok(()),
            Err(sys::EINTR) => continue,
            Err(errno) => break Err(io::Error::from_raw_os_error(errno)),
        }
    }
}

pub fn pipe_fail(why: &io::Error) {
    eprintln!("ion: failed to create pipe: {:?}", why);
}

pub fn append_external_stdio_pipe(pipes: &mut Option<Vec<File>>, file: RawFd) {
    pipes.get_or_insert_with(|| Vec::with_capacity(4)).push(unsafe { File::from_raw_fd(file) });
}
