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

pub use self::pipes::create_pipe;
use self::{job_control::ProcessState, pipes::TeePipe};
use super::{
    job::{RefinedJob, TeeItem, Variant},
    signals::{self, SignalHandler},
    IonError, Shell, Value,
};
use crate::{
    builtins::Status,
    expansion::pipelines::{Input, PipeItem, PipeType, Pipeline, RedirectFrom, Redirection},
    types,
};
use nix::{
    sys::signal::{self, Signal},
    unistd::{self, ForkResult, Pid},
};
use smallvec::SmallVec;
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    os::unix::process::CommandExt,
    process::{exit, Command, Stdio},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RedirectError {
    /// Input
    #[error("failed to redirect '{0}' to stdin: {1}")]
    File(String, #[source] io::Error),
    #[error("failed to write herestring '{0}': {1}")]
    WriteError(String, #[source] io::Error),

    /// Output
    #[error("failed to redirect {redirect} to file '{file}': {why}")]
    Output {
        redirect: RedirectFrom,
        file:     String,
        #[source]
        why:      io::Error,
    },
}

/// This is created when Ion fails to create a pipeline
#[derive(Debug, Error)]
pub enum PipelineError {
    /// The fork failed
    #[error("failed to fork: {0}")]
    Fork(#[source] nix::Error),
    /// Failed to setup capturing for function
    #[error("error reading stdout of child: {0}")]
    CaptureFailed(#[source] io::Error),

    /// Failed to duplicate a file descriptor
    #[error("could not duplicate the pipe: {0}")]
    CloneFdFailed(#[source] nix::Error),

    /// Could not clone the file
    #[error("could not clone the pipe: {0}")]
    ClonePipeFailed(#[source] io::Error),
    /// Could not set the pipe as a redirection
    #[error("{0}")]
    RedirectPipeError(#[source] RedirectError),
    /// Failed to create a pipe
    #[error("could not create pipe: {0}")]
    CreatePipeError(#[source] nix::Error),
    /// Failed to create a fork
    #[error("could not fork: {0}")]
    CreateForkError(#[source] nix::Error),
    /// Failed to terminate the jobs after a termination
    #[error("failed to terminate foreground jobs: {0}")]
    TerminateJobsError(#[source] nix::Error),
    /// Could not execute the command
    #[error("command exec error: {0}")]
    CommandExecError(#[source] io::Error, types::Args),
    /// Could not expand the alias
    #[error("unable to pipe outputs of alias: '{0} = {1}'")]
    InvalidAlias(String, String),

    /// A signal interrupted a child process
    #[error("process ({0}) ended by signal {1}")]
    Interrupted(Pid, Signal),
    /// A subprocess had a core dump
    #[error("process ({0}) had a core dump")]
    CoreDump(Pid),
    /// WaitPID errored
    #[error("waitpid error: {0}")]
    WaitPid(nix::Error),

    /// This will stop execution when the exit_on_error option is set
    #[error("early exit: pipeline failed with error code {0}")]
    EarlyExit(Status),

    /// A command could not be found in the pipeline
    #[error("command not found: {0}")]
    CommandNotFound(types::Str),

    /// Failed to grab the tty
    #[error("could not grab the terminal: {0}")]
    TerminalGrabFailed(#[source] nix::Error),

    /// Failed to send signal to a process group. This typically happens when trying to start the
    /// pipeline after it's creation
    #[error("could not kill the processes: {0}")]
    KillFailed(#[source] nix::Error),
}

impl From<RedirectError> for PipelineError {
    fn from(cause: RedirectError) -> Self { Self::RedirectPipeError(cause) }
}

/// Create an OS pipe and write the contents of a byte slice to one end
/// such that reading from this pipe will produce the byte slice. Return
/// A file descriptor representing the read end of the pipe.
pub fn stdin_of<T: AsRef<str>>(input: &T) -> Result<File, PipelineError> {
    let string = input.as_ref();
    let (reader, mut writer) = create_pipe()?;
    // Write the contents; make sure to use write_all so that we block until
    // the entire string is written
    writer
        .write_all(string.as_bytes())
        .map_err(|err| RedirectError::WriteError(string.into(), err))?;
    if !string.ends_with('\n') {
        writer.write(b"\n").map_err(|err| RedirectError::WriteError(string.into(), err))?;
    }
    writer.flush().map_err(|err| RedirectError::WriteError(string.into(), err))?;
    // `infile` currently owns the writer end RawFd. If we just return the reader
    // end and let `infile` go out of scope, it will be closed, sending EOF to
    // the reader!
    Ok(reader)
}

impl Input {
    pub(self) fn get_infile(&self) -> Result<File, PipelineError> {
        match self {
            Self::File(ref filename) => match File::open(filename.as_str()) {
                Ok(file) => Ok(file),
                Err(why) => Err(RedirectError::File(filename.to_string(), why).into()),
            },
            Self::HereString(ref string) => stdin_of(&string),
        }
    }
}

fn need_tee(outs: &[Redirection], redirection: RedirectFrom) -> (bool, bool) {
    let (mut stdout_count, mut stderr_count) = match redirection {
        RedirectFrom::Both => (1, 1),
        RedirectFrom::Stdout => (1, 0),
        RedirectFrom::Stderr => (0, 1),
        RedirectFrom::None => (0, 0),
    };

    for &Redirection { from, .. } in outs {
        match from {
            RedirectFrom::Both => {
                stdout_count += 1;
                stderr_count += 1;
            }
            RedirectFrom::Stdout => stdout_count += 1,
            RedirectFrom::Stderr => stderr_count += 1,
            RedirectFrom::None => (),
        }
        if stdout_count > 1 && stderr_count > 1 {
            return (true, true);
        }
    }
    (stdout_count > 1, stderr_count > 1)
}

fn do_tee<'a>(
    outputs: &[Redirection],
    job: &mut RefinedJob<'a>,
    stdout: &mut dyn FnMut(&mut RefinedJob<'a>, File),
    stderr: &mut dyn FnMut(&mut RefinedJob<'a>, File),
) -> Result<(), RedirectError> {
    // XXX: Possibly add an assertion here for correctness
    for output in outputs {
        match OpenOptions::new()
            .create(true)
            .write(true)
            .append(output.append)
            .truncate(!output.append)
            .open(output.file.as_str())
        {
            Ok(file) => match output.from {
                RedirectFrom::None => (),
                RedirectFrom::Stdout => stdout(job, file),
                RedirectFrom::Stderr => stderr(job, file),
                RedirectFrom::Both => match file.try_clone() {
                    Ok(f_copy) => {
                        stdout(job, file);
                        stderr(job, f_copy);
                    }
                    Err(why) => {
                        return Err(RedirectError::Output {
                            redirect: output.from,
                            file: output.file.to_string(),
                            why,
                        });
                    }
                },
            },
            Err(why) => {
                return Err(RedirectError::Output {
                    redirect: output.from,
                    file: output.file.to_string(),
                    why,
                });
            }
        }
    }
    Ok(())
}

/// Insert the multiple redirects as pipelines if necessary. Handle both input and output
/// redirection if necessary.
fn prepare<'a>(
    pipeline: Pipeline<RefinedJob<'a>>,
) -> Result<impl IntoIterator<Item = RefinedJob<'a>>, PipelineError> {
    // Real logic begins here
    let mut new_commands =
        SmallVec::<[RefinedJob<'a>; 16]>::with_capacity(2 * pipeline.items.len());
    let mut prev_kind = RedirectFrom::None;
    for PipeItem { mut job, outputs, inputs } in pipeline.items {
        let kind = job.redirection;
        match (inputs.len(), prev_kind) {
            (0, _) => {}
            (1, RedirectFrom::None) => job.stdin(inputs[0].get_infile()?),
            _ => {
                new_commands.push(RefinedJob::cat(
                    inputs.iter().map(Input::get_infile).collect::<Result<_, _>>()?,
                    RedirectFrom::Stdout,
                ));
            }
        }
        prev_kind = kind;
        if outputs.is_empty() {
            new_commands.push(job);
        } else {
            match need_tee(&outputs, kind) {
                // No tees
                (false, false) => {
                    do_tee(&outputs, &mut job, &mut RefinedJob::stdout, &mut RefinedJob::stderr)?;
                    new_commands.push(job);
                }
                // tee stderr
                (false, true) => {
                    let mut tee = TeeItem::new();
                    do_tee(&outputs, &mut job, &mut RefinedJob::stdout, &mut |_, f| tee.add(f))?;
                    let tee = RefinedJob::tee(None, Some(tee), job.redirection);
                    job.redirection = RedirectFrom::Stderr;
                    new_commands.push(job);
                    new_commands.push(tee);
                }
                // tee stdout
                (true, false) => {
                    let mut tee = TeeItem::new();
                    do_tee(&outputs, &mut job, &mut |_, f| tee.add(f), &mut RefinedJob::stderr)?;
                    let tee = RefinedJob::tee(Some(tee), None, job.redirection);
                    job.redirection = RedirectFrom::Stdout;
                    new_commands.push(job);
                    new_commands.push(tee);
                }
                // tee both
                (true, true) => {
                    let mut tee_out = TeeItem::new();
                    let mut tee_err = TeeItem::new();
                    do_tee(&outputs, &mut job, &mut |_, f| tee_out.add(f), &mut |_, f| {
                        tee_err.sinks.push(f)
                    })?;
                    let tee = RefinedJob::tee(Some(tee_out), Some(tee_err), job.redirection);
                    job.redirection = RedirectFrom::Stdout;
                    new_commands.push(job);
                    new_commands.push(tee);
                }
            }
        }
    }
    Ok(new_commands)
}

impl<'b> Shell<'b> {
    /// For tee jobs
    fn exec_multi_out(
        items: &mut (Option<TeeItem>, Option<TeeItem>),
        redirection: RedirectFrom,
    ) -> Status {
        let res = match *items {
            (None, None) => panic!("There must be at least one TeeItem, this is a bug"),
            (Some(ref mut tee_out), None) => match redirection {
                RedirectFrom::Stderr | RedirectFrom::None => tee_out.write_to_all(None),
                _ => tee_out.write_to_all(Some(RedirectFrom::Stdout)),
            },
            (None, Some(ref mut tee_err)) => match redirection {
                RedirectFrom::Stdout | RedirectFrom::None => tee_err.write_to_all(None),
                _ => tee_err.write_to_all(Some(RedirectFrom::Stderr)),
            },
            // TODO Make it work with pipes
            (Some(ref mut tee_out), Some(ref mut tee_err)) => {
                tee_out.write_to_all(None).and_then(|_| tee_err.write_to_all(None))
            }
        };
        if let Err(e) = res {
            Status::error(format!("ion: error in multiple output redirection process: {:?}", e))
        } else {
            Status::SUCCESS
        }
    }

    /// For cat jobs
    fn exec_multi_in(sources: &mut [File], stdin: &mut Option<File>) -> Status {
        let stdout = io::stdout();
        let mut stdout = stdout.lock();
        for file in stdin.iter_mut().chain(sources) {
            if let Err(why) = std::io::copy(file, &mut stdout) {
                return Status::error(format!(
                    "ion: error in multiple input redirect process: {:?}",
                    why
                ));
            }
        }
        Status::SUCCESS
    }

    fn exec_function<S: AsRef<str>>(&mut self, name: &str, args: &[S]) -> Result<Status, IonError> {
        if let Some(Value::Function(function)) = self.variables.get(name).cloned() {
            function.execute(self, args).map(|_| self.previous_status)
        } else {
            unreachable!()
        }
    }

    /// Executes a `RefinedJob` that was created in the `generate_commands` method.
    ///
    /// The aforementioned `RefinedJob` may be either a builtin or external command.
    /// The purpose of this function is therefore to execute both types accordingly.
    fn exec_job(&mut self, job: &RefinedJob<'b>) -> Result<Status, IonError> {
        // Duplicate file descriptors, execute command, and redirect back.
        let (stdin_bk, stdout_bk, stderr_bk) =
            streams::duplicate().map_err(PipelineError::CreatePipeError)?;
        streams::redirect(&job.stdin, &job.stdout, &job.stderr)?;
        let code = match job.var {
            Variant::Builtin { main } => Ok(main(job.args(), self)),
            Variant::Function => self.exec_function(job.command(), job.args()),
            _ => panic!("exec job should not be able to be called on Cat or Tee jobs"),
        };
        streams::redirect(&stdin_bk, &Some(stdout_bk), &Some(stderr_bk))?;
        code
    }

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
    pub fn execute_pipeline(
        &mut self,
        pipeline: Pipeline<RefinedJob<'b>>,
    ) -> Result<Status, IonError> {
        // While active, the SIGTTOU signal will be ignored.
        let _sig_ignore = SignalHandler::new();

        // If the given pipeline is a background task, fork the shell.
        match pipeline.pipe {
            PipeType::Disown => Ok(self.fork_pipe(pipeline, ProcessState::Empty)),
            PipeType::Background => Ok(self.fork_pipe(pipeline, ProcessState::Running)),
            // Execute each command in the pipeline, giving each command the foreground.
            PipeType::Normal => {
                let exit_status = self.pipe(pipeline);
                // Set the shell as the foreground process again to regain the TTY.
                if self.opts.grab_tty {
                    let _ = unistd::tcsetpgrp(0, Pid::this());
                }
                exit_status
            }
        }
    }

    /// Executes a piped job `job1 | job2 | job3`
    ///
    /// This function will panic if called with an empty slice
    fn pipe(&mut self, pipeline: Pipeline<RefinedJob<'b>>) -> Result<Status, IonError> {
        let mut commands = prepare(pipeline)?.into_iter().peekable();

        if let Some(mut parent) = commands.next() {
            if parent.redirection == RedirectFrom::None && !parent.needs_forking() {
                let status = self.exec_job(&parent);

                let _ = io::stdout().flush();
                let _ = io::stderr().flush();

                status
            } else {
                let (mut pgid, mut last_pid, mut current_pid) = (None, None, Pid::this());

                // Append jobs until all piped jobs are running
                for mut child in commands {
                    // Keep a reference to the FD to keep them open
                    let mut ext_stdio_pipes: Option<Vec<File>> = None;

                    // If parent is a RefindJob::External, then we need to keep track of the
                    // output pipes, so we can properly close them after the job has been
                    // spawned.
                    let is_external = matches!(parent.var, Variant::External { .. });

                    // TODO: Refactor this part
                    // If we need to tee both stdout and stderr, we directly connect pipes to
                    // the relevant sources in both of them.
                    if let Variant::Tee {
                        items: (Some(ref mut tee_out), Some(ref mut tee_err)),
                        ..
                    } = child.var
                    {
                        TeePipe::new(&mut parent, &mut ext_stdio_pipes, is_external)
                            .connect(tee_out, tee_err)?;
                    } else {
                        // Pipe the previous command's stdin to this commands stdout/stderr.
                        let (reader, writer) = create_pipe()?;
                        if is_external {
                            ext_stdio_pipes
                                .get_or_insert_with(|| Vec::with_capacity(4))
                                .push(writer.try_clone().map_err(PipelineError::ClonePipeFailed)?);
                        }
                        child.stdin(reader);
                        match parent.redirection {
                            RedirectFrom::None => (),
                            RedirectFrom::Stderr => parent.stderr(writer),
                            RedirectFrom::Stdout => parent.stdout(writer),
                            RedirectFrom::Both => {
                                let duped = writer.try_clone().map_err(|why| {
                                    PipelineError::RedirectPipeError(RedirectError::Output {
                                        redirect: parent.redirection,
                                        file: "pipe".to_string(),
                                        why,
                                    })
                                })?;
                                parent.stderr(writer);
                                parent.stdout(duped);
                            }
                        }
                    }

                    spawn_proc(self, parent, &mut last_pid, &mut current_pid, &mut pgid)?;

                    last_pid = Some(current_pid);
                    parent = child;
                    if parent.redirection == RedirectFrom::None {
                        break;
                    }
                }

                spawn_proc(self, parent, &mut last_pid, &mut current_pid, &mut pgid)?;
                if self.opts.grab_tty {
                    unistd::tcsetpgrp(nix::libc::STDIN_FILENO, pgid.unwrap())
                        .map_err(PipelineError::TerminalGrabFailed)?;
                }
                let _ = signal::killpg(pgid.unwrap(), signal::Signal::SIGCONT);

                // Waits for all of the children of the assigned pgid to finish executing,
                // returning the exit status of the last process in the queue.
                // Watch the foreground group, dropping all commands that exit as they exit.
                let status = self.watch_foreground(pgid.unwrap())?;
                if status == Status::TERMINATED {
                    signal::killpg(pgid.unwrap(), signal::Signal::SIGTERM)
                        .map_err(PipelineError::TerminateJobsError)?;
                } else {
                    let _ = io::stdout().flush();
                    let _ = io::stderr().flush();
                }
                Ok(status)
            }
        } else {
            Ok(Status::SUCCESS)
        }
    }
}

fn spawn_proc(
    shell: &mut Shell<'_>,
    cmd: RefinedJob<'_>,
    last_pid: &mut Option<Pid>,
    current_pid: &mut Pid,
    group: &mut Option<Pid>,
) -> Result<(), PipelineError> {
    let RefinedJob { mut var, mut args, stdin, stdout, stderr, redirection } = cmd;
    let pid = match var {
        Variant::External => {
            let mut command = Command::new(&args[0].as_str());
            command.args(args[1..].iter().map(types::Str::as_str));

            command.stdin(stdin.map_or_else(Stdio::inherit, Into::into));
            command.stdout(stdout.map_or_else(Stdio::inherit, Into::into));
            command.stderr(stderr.map_or_else(Stdio::inherit, Into::into));

            let grp = *group;
            unsafe {
                command.pre_exec(move || {
                    signals::unblock();
                    let _ = unistd::setpgid(Pid::this(), grp.unwrap_or_else(Pid::this));
                    Ok(())
                })
            };
            match command.spawn() {
                Ok(child) => Ok(Pid::from_raw(child.id() as i32)),
                Err(err) => {
                    if err.kind() == io::ErrorKind::NotFound {
                        Err(PipelineError::CommandNotFound(args.swap_remove(0)))
                    } else {
                        Err(PipelineError::CommandExecError(err, args))
                    }
                }
            }
        }
        Variant::Builtin { main } => {
            fork_exec_internal(stdout, stderr, stdin, *group, |_, _, _| main(&args, shell))
        }
        Variant::Function => fork_exec_internal(stdout, stderr, stdin, *group, |_, _, _| {
            shell
                .exec_function(&args[0], &args)
                .unwrap_or_else(|why| Status::error(format!("{}", why)))
        }),
        Variant::Cat { ref mut sources } => {
            fork_exec_internal(stdout, None, stdin, *group, |_, _, mut stdin| {
                Shell::exec_multi_in(sources, &mut stdin)
            })
        }
        Variant::Tee { ref mut items } => {
            fork_exec_internal(stdout, stderr, stdin, *group, |_, _, _| {
                Shell::exec_multi_out(items, redirection)
            })
        }
    }?;
    *last_pid = Some(std::mem::replace(current_pid, pid));
    if group.is_none() {
        *group = Some(pid);
    }
    let _ = unistd::setpgid(pid, group.unwrap()); // try in the parent too to avoid race conditions
    Ok(())
}

// TODO: Integrate this better within the RefinedJob type.
fn fork_exec_internal<F>(
    stdout: Option<File>,
    stderr: Option<File>,
    stdin: Option<File>,
    pgid: Option<Pid>,
    mut exec_action: F,
) -> Result<Pid, PipelineError>
where
    F: FnMut(Option<File>, Option<File>, Option<File>) -> Status,
{
    match unsafe { unistd::fork().map_err(PipelineError::CreateForkError)? } {
        ForkResult::Child => {
            unsafe {
                signal::signal(signal::Signal::SIGINT, signal::SigHandler::SigIgn).unwrap();
                signal::signal(signal::Signal::SIGHUP, signal::SigHandler::SigIgn).unwrap();
                signal::signal(signal::Signal::SIGTERM, signal::SigHandler::SigIgn).unwrap();
            }
            signals::unblock();

            unistd::setpgid(Pid::this(), pgid.unwrap_or_else(Pid::this)).unwrap();
            streams::redirect(&stdin, &stdout, &stderr).unwrap();
            let exit_status = exec_action(stdout, stderr, stdin);
            exit(exit_status.as_os_code())
        }
        ForkResult::Parent { child } => Ok(child),
    }
}
