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

use self::{job_control::ProcessState, pipes::TeePipe};
use super::{
    flow_control::FunctionError,
    job::{Job, RefinedJob, TeeItem, Variant},
    signals::{self, SignalHandler},
    Shell, Value,
};
use crate::{
    builtins::{self, Status},
    expansion::pipelines::{Input, PipeItem, PipeType, Pipeline, RedirectFrom, Redirection},
    types,
};
use err_derive::Error;
use nix::{
    fcntl::OFlag,
    sys::signal::{self, Signal},
    unistd::{self, ForkResult, Pid},
};
use smallvec::SmallVec;
use std::{
    fmt,
    fs::{File, OpenOptions},
    io::{self, Write},
    iter,
    os::unix::{io::FromRawFd, process::CommandExt},
    path::Path,
    process::{exit, Command, Stdio},
};

#[derive(Debug, Error)]
pub enum InputError {
    #[error(display = "failed to redirect '{}' to stdin: {}", _0, _1)]
    File(String, #[error(cause)] io::Error),
    #[error(display = "failed to redirect herestring '{}' to stdin: {}", _0, _1)]
    HereString(String, #[error(cause)] nix::Error),
    #[error(display = "failed to redirect herestring '{}' to stdin: {}", _0, _1)]
    WriteError(String, #[error(cause)] io::Error),
}

#[derive(Debug)]
pub struct OutputError {
    redirect: RedirectFrom,
    file:     String,
    why:      io::Error,
}

#[derive(Debug, Error)]
pub enum RedirectError {
    #[error(display = "{}", _0)]
    Input(#[error(cause)] InputError),
    #[error(display = "{}", _0)]
    Output(#[error(cause)] OutputError),
}

/// This is created when Ion fails to create a pipeline
#[derive(Debug, Error)]
pub enum PipelineError {
    /// The fork failed
    #[error(display = "failed to fork: {}", _0)]
    Fork(#[error(cause)] nix::Error),
    /// Failed to setup capturing for function
    #[error(display = "error reading stdout of child: {}", _0)]
    CaptureFailed(#[error(cause)] io::Error),

    /// Could not set the pipe as a redirection
    #[error(display = "{}", _0)]
    RedirectPipeError(#[error(cause)] RedirectError),
    /// Failed to create a pipe
    #[error(display = "could not create pipe: {}", _0)]
    CreatePipeError(#[error(cause)] nix::Error),
    /// Failed to create a fork
    #[error(display = "could not fork: {}", _0)]
    CreateForkError(#[error(cause)] nix::Error),
    /// Failed to run function
    #[error(display = "could not run function: {}", _0)]
    RunFunctionError(#[error(cause)] FunctionError),
    /// Failed to terminate the jobs after a termination
    #[error(display = "failed to terminate foreground jobs: {}", _0)]
    TerminateJobsError(#[error(cause)] nix::Error),
    /// Could not execute the command
    #[error(display = "command exec error: {}", _0)]
    CommandExecError(#[error(cause)] io::Error),
    /// Could not expand the alias
    #[error(display = "unable to pipe outputs of alias: '{} = {}'", _0, _1)]
    InvalidAlias(String, String),

    /// A signal interrupted a child process
    #[error(display = "process ({}) ended by signal {}", _0, _1)]
    Interrupted(Pid, Signal),
    /// A subprocess had a core dump
    #[error(display = "process ({}) had a core dump", _0)]
    CoreDump(Pid),
    /// WaitPID errored
    #[error(display = "waitpid error: {}", _0)]
    WaitPid(nix::Error),

    /// This will stop execution when the exit_on_error option is set
    #[error(display = "early exit: pipeline failed")]
    EarlyExit,

    /// A command could not be found in the pipeline
    #[error(display = "command not found: {}", _0)]
    CommandNotFound(String),

    /// Failed to grab the tty
    #[error(display = "could not grab the terminal: {}", _0)]
    TerminalGrabFailed(#[error(cause)] nix::Error),

    /// Failed to send signal to a process group. This typically happens when trying to start the
    /// pipeline after it's creation
    #[error(display = "could not start the processes: {}", _0)]
    KillFailed(#[error(cause)] nix::Error),
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "failed to redirect {} to file '{}': {}",
            match self.redirect {
                RedirectFrom::Both => "both stdout and stderr",
                RedirectFrom::Stdout => "stdout",
                RedirectFrom::Stderr => "stderr",
                _ => unreachable!(),
            },
            self.file,
            self.why,
        )
    }
}

impl std::error::Error for OutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { Some(&self.why) }
}

impl From<OutputError> for RedirectError {
    fn from(cause: OutputError) -> Self { RedirectError::Output(cause) }
}

impl From<InputError> for RedirectError {
    fn from(cause: InputError) -> Self { RedirectError::Input(cause) }
}

impl From<RedirectError> for PipelineError {
    fn from(cause: RedirectError) -> Self { PipelineError::RedirectPipeError(cause) }
}

impl From<FunctionError> for PipelineError {
    fn from(cause: FunctionError) -> Self { PipelineError::RunFunctionError(cause) }
}

/// Create an OS pipe and write the contents of a byte slice to one end
/// such that reading from this pipe will produce the byte slice. Return
/// A file descriptor representing the read end of the pipe.
pub unsafe fn stdin_of<T: AsRef<str>>(input: &T) -> Result<File, InputError> {
    let string = input.as_ref();
    let (reader, writer) = unistd::pipe2(OFlag::O_CLOEXEC)
        .map_err(|err| InputError::HereString(string.into(), err))?;
    let mut infile = File::from_raw_fd(writer);
    // Write the contents; make sure to use write_all so that we block until
    // the entire string is written
    infile
        .write_all(string.as_bytes())
        .map_err(|err| InputError::WriteError(string.into(), err))?;
    infile.flush().map_err(|err| InputError::WriteError(string.into(), err))?;
    // `infile` currently owns the writer end RawFd. If we just return the reader
    // end and let `infile` go out of scope, it will be closed, sending EOF to
    // the reader!
    Ok(File::from_raw_fd(reader))
}

impl Input {
    pub(self) fn get_infile(&mut self) -> Result<File, InputError> {
        match self {
            Input::File(ref filename) => match File::open(filename.as_str()) {
                Ok(file) => Ok(file),
                Err(why) => Err(InputError::File(filename.to_string(), why)),
            },
            Input::HereString(ref mut string) => {
                if !string.ends_with('\n') {
                    string.push('\n');
                }

                unsafe { stdin_of(&string) }
            }
        }
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
) -> Result<(), OutputError> {
    // XXX: Possibly add an assertion here for correctness
    for output in outputs {
        match OpenOptions::new()
            .create(true)
            .write(true)
            .append(output.append)
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
                        return Err(OutputError {
                            redirect: output.from,
                            file: output.file.to_string(),
                            why,
                        });
                    }
                },
            },
            Err(why) => {
                return Err(OutputError {
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
fn prepare<'a, 'b>(
    shell: &'a Shell<'b>,
    pipeline: Pipeline<'b>,
) -> Result<impl IntoIterator<Item = (RefinedJob<'b>, RedirectFrom)>, RedirectError> {
    // Real logic begins here
    let mut new_commands = SmallVec::<[_; 16]>::with_capacity(2 * pipeline.items.len());
    let mut prev_kind = RedirectFrom::None;
    for PipeItem { job, outputs, mut inputs } in pipeline.items {
        let kind = job.redirection;
        let mut job = shell.generate_command(job);
        match (inputs.len(), prev_kind) {
            (0, _) => {}
            (1, RedirectFrom::None) => job.stdin(inputs[0].get_infile()?),
            _ => {
                new_commands.push((
                    RefinedJob::cat(
                        inputs.iter_mut().map(Input::get_infile).collect::<Result<_, _>>()?,
                    ),
                    RedirectFrom::Stdout,
                ));
            }
        }
        prev_kind = kind;
        if outputs.is_empty() {
            new_commands.push((job, kind));
        } else {
            match need_tee(&outputs, kind) {
                // No tees
                (false, false) => {
                    do_tee(&outputs, &mut job, &mut RefinedJob::stdout, &mut RefinedJob::stderr)?;
                    new_commands.push((job, kind));
                }
                // tee stderr
                (false, true) => {
                    let mut tee = TeeItem::new();
                    do_tee(&outputs, &mut job, &mut RefinedJob::stdout, &mut |_, f| tee.add(f))?;
                    new_commands.push((job, RedirectFrom::Stderr));
                    new_commands.push((RefinedJob::tee(None, Some(tee)), kind));
                }
                // tee stdout
                (true, false) => {
                    let mut tee = TeeItem::new();
                    do_tee(&outputs, &mut job, &mut |_, f| tee.add(f), &mut RefinedJob::stderr)?;
                    new_commands.push((job, RedirectFrom::Stdout));
                    new_commands.push((RefinedJob::tee(Some(tee), None), kind));
                }
                // tee both
                (true, true) => {
                    let mut tee_out = TeeItem::new();
                    let mut tee_err = TeeItem::new();
                    do_tee(&outputs, &mut job, &mut |_, f| tee_out.add(f), &mut |_, f| {
                        tee_err.sinks.push(f)
                    })?;
                    new_commands.push((job, RedirectFrom::Stdout));
                    new_commands.push((RefinedJob::tee(Some(tee_out), Some(tee_err)), kind));
                }
            }
        }
    }
    Ok(new_commands)
}

impl<'b> Shell<'b> {
    /// For tee jobs
    fn exec_multi_out(
        &mut self,
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
    fn exec_multi_in(&mut self, sources: &mut [File], stdin: &mut Option<File>) -> Status {
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

    fn exec_function<S: AsRef<str>>(&mut self, name: &str, args: &[S]) -> Status {
        if let Some(Value::Function(function)) = self.variables.get(name).cloned() {
            match function.execute(self, args) {
                Ok(()) => Status::SUCCESS,
                Err(why) => Status::error(format!("{}", why)),
            }
        } else {
            unreachable!()
        }
    }

    /// Executes a `RefinedJob` that was created in the `generate_commands` method.
    ///
    /// The aforementioned `RefinedJob` may be either a builtin or external command.
    /// The purpose of this function is therefore to execute both types accordingly.
    fn exec_job(&mut self, job: &RefinedJob<'b>) -> Result<Status, PipelineError> {
        // Duplicate file descriptors, execute command, and redirect back.
        let (stdin_bk, stdout_bk, stderr_bk) =
            streams::duplicate().map_err(PipelineError::CreatePipeError)?;
        streams::redirect(&job.stdin, &job.stdout, &job.stderr);
        let code = match job.var {
            Variant::Builtin { main } => main(job.args(), self),
            Variant::Function => self.exec_function(job.command(), job.args()),
            _ => panic!("exec job should not be able to be called on Cat or Tee jobs"),
        };
        streams::redirect(&stdin_bk, &Some(stdout_bk), &Some(stderr_bk));
        Ok(code)
    }

    /// Generates a vector of commands from a given `Pipeline`.
    ///
    /// Each generated command will either be a builtin or external command, and will be
    /// associated will be marked as an `&&`, `||`, `|`, or final job.
    fn generate_command(&self, job: Job<'b>) -> RefinedJob<'b> {
        if is_implicit_cd(&job.args[0]) {
            RefinedJob::builtin(
                &builtins::builtin_cd,
                iter::once("cd".into()).chain(job.args).collect(),
            )
        } else if let Some(Value::Function(_)) = self.variables.get(&job.args[0]) {
            RefinedJob::function(job.args)
        } else if let Some(builtin) = job.builtin {
            RefinedJob::builtin(builtin, job.args)
        } else {
            RefinedJob::external(job.args)
        }
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
    pub fn execute_pipeline(&mut self, pipeline: Pipeline<'b>) -> Result<Status, PipelineError> {
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
                if !self.opts.is_background_shell {
                    let _ = unistd::tcsetpgrp(0, Pid::this());
                }
                exit_status
            }
        }
    }

    /// Executes a piped job `job1 | job2 | job3`
    ///
    /// This function will panic if called with an empty slice
    fn pipe(&mut self, pipeline: Pipeline<'b>) -> Result<Status, PipelineError> {
        let mut commands = prepare(self, pipeline)?.into_iter().peekable();

        if let Some((mut parent, mut kind)) = commands.next() {
            if kind == RedirectFrom::None && !parent.needs_forking() {
                let status = self.exec_job(&parent);

                let _ = io::stdout().flush();
                let _ = io::stderr().flush();

                status
            } else {
                let (mut pgid, mut last_pid, mut current_pid) = (None, None, Pid::this());

                // Append jobs until all piped jobs are running
                for (mut child, ckind) in commands {
                    // Keep a reference to the FD to keep them open
                    let mut ext_stdio_pipes: Option<Vec<File>> = None;

                    // If parent is a RefindJob::External, then we need to keep track of the
                    // output pipes, so we can properly close them after the job has been
                    // spawned.
                    let is_external =
                        if let Variant::External { .. } = parent.var { true } else { false };

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
                        let (reader, writer) = unistd::pipe2(nix::fcntl::OFlag::O_CLOEXEC)
                            .map_err(PipelineError::CreatePipeError)?;
                        if is_external {
                            ext_stdio_pipes
                                .get_or_insert_with(|| Vec::with_capacity(4))
                                .push(unsafe { File::from_raw_fd(writer) });
                        }
                        child.stdin(unsafe { File::from_raw_fd(reader) });
                        let writer = unsafe { File::from_raw_fd(writer) };
                        match kind {
                            RedirectFrom::None => (),
                            RedirectFrom::Stderr => parent.stderr(writer),
                            RedirectFrom::Stdout => parent.stdout(writer),
                            RedirectFrom::Both => {
                                let duped = writer.try_clone().map_err(|why| {
                                    RedirectError::from(OutputError {
                                        redirect: kind,
                                        file: "pipe".to_string(),
                                        why,
                                    })
                                })?;
                                parent.stderr(writer);
                                parent.stdout(duped);
                            }
                        }
                    }

                    spawn_proc(self, parent, kind, &mut last_pid, &mut current_pid, &mut pgid)?;

                    last_pid = Some(current_pid);
                    parent = child;
                    kind = ckind;
                    if ckind == RedirectFrom::None {
                        break;
                    }
                }

                spawn_proc(self, parent, kind, &mut last_pid, &mut current_pid, &mut pgid)?;
                if !self.opts.is_background_shell {
                    unistd::tcsetpgrp(nix::libc::STDIN_FILENO, pgid.unwrap())
                        .map_err(PipelineError::TerminalGrabFailed)?;
                }
                signal::killpg(pgid.unwrap(), signal::Signal::SIGCONT)
                    .map_err(PipelineError::KillFailed)?;

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
    redirection: RedirectFrom,
    last_pid: &mut Option<Pid>,
    current_pid: &mut Pid,
    group: &mut Option<Pid>,
) -> Result<(), PipelineError> {
    let RefinedJob { mut var, args, stdin, stdout, stderr } = cmd;
    let pid = match var {
        Variant::External => {
            let mut command = Command::new(&args[0].as_str());
            command.args(args[1..].iter().map(types::Str::as_str));

            command.stdin(stdin.map_or_else(Stdio::inherit, Into::into));
            command.stdout(stdout.map_or_else(Stdio::inherit, Into::into));
            command.stderr(stderr.map_or_else(Stdio::inherit, Into::into));

            let grp = *group;
            command.before_exec(move || {
                let _ = unistd::setpgid(Pid::this(), grp.unwrap_or_else(Pid::this));
                Ok(())
            });
            match command.spawn() {
                Ok(child) => Ok(Pid::from_raw(child.id() as i32)),
                Err(err) => {
                    if err.kind() == io::ErrorKind::NotFound {
                        Err(PipelineError::CommandNotFound(args[0].to_string()))
                    } else {
                        Err(PipelineError::CommandExecError(err))
                    }
                }
            }
        }
        Variant::Builtin { main } => {
            fork_exec_internal(stdout, stderr, stdin, *group, |_, _, _| main(&args, shell))
        }
        Variant::Function => fork_exec_internal(stdout, stderr, stdin, *group, |_, _, _| {
            shell.exec_function(&args[0], &args)
        }),
        Variant::Cat { ref mut sources } => {
            fork_exec_internal(stdout, None, stdin, *group, |_, _, mut stdin| {
                shell.exec_multi_in(sources, &mut stdin)
            })
        }
        Variant::Tee { ref mut items } => {
            fork_exec_internal(stdout, stderr, stdin, *group, |_, _, _| {
                shell.exec_multi_out(items, redirection)
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
    match unistd::fork().map_err(PipelineError::CreateForkError)? {
        ForkResult::Child => {
            unsafe {
                signal::signal(signal::Signal::SIGINT, signal::SigHandler::SigIgn).unwrap();
                signal::signal(signal::Signal::SIGHUP, signal::SigHandler::SigIgn).unwrap();
                signal::signal(signal::Signal::SIGTERM, signal::SigHandler::SigIgn).unwrap();
            }
            signals::unblock();

            unistd::setpgid(Pid::this(), pgid.unwrap_or_else(Pid::this)).unwrap();
            streams::redirect(&stdin, &stdout, &stderr);
            let exit_status = exec_action(stdout, stderr, stdin);
            exit(exit_status.as_os_code())
        }
        ForkResult::Parent { child } => Ok(child),
    }
}
