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
    job_control::ProcessState,
    pipes::TeePipe,
    streams::{duplicate_streams, redirect_streams},
};
use super::{
    flow_control::FunctionError,
    job::{Job, JobVariant, RefinedJob, TeeItem},
    signals::{self, SignalHandler},
    status::Status,
    Shell, Value,
};
use crate::{
    builtins,
    parser::pipelines::{Input, PipeItem, PipeType, Pipeline, RedirectFrom, Redirection},
    sys,
};
use err_derive::Error;
use smallvec::SmallVec;
use std::{
    fmt,
    fs::{File, OpenOptions},
    io::{self, Write},
    iter,
    os::unix::io::{AsRawFd, FromRawFd, RawFd},
    path::Path,
    process::{self, exit},
};

#[derive(Debug, Error)]
pub enum InputError {
    #[error(display = "failed to redirect '{}' to stdin: {}", file, why)]
    File { file: String, why: io::Error },
    #[error(display = "failed to redirect herestring '{}' to stdin: {}", string, why)]
    HereString { string: String, why: io::Error },
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

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error(display = "{}", _0)]
    RedirectPipeError(#[error(cause)] RedirectError),
    #[error(display = "could not create pipe: {}", _0)]
    CreatePipeError(#[error(cause)] io::Error),
    #[error(display = "could not fork: {}", _0)]
    CreateForkError(#[error(cause)] io::Error),
    #[error(display = "could not run function: {}", _0)]
    RunFunctionError(#[error(cause)] FunctionError),
    #[error(display = "failed to terminate foreground jobs: {}", _0)]
    TerminateJobsError(#[error(cause)] io::Error),
    #[error(display = "command exec error: {}", _0)]
    CommandExecError(#[error(cause)] io::Error),
    #[error(display = "unable to pipe outputs of alias: '{} = {}'", _0, _1)]
    InvalidAlias(String, String),

    #[error(display = "process ({}) ended by signal {}", _0, _1)]
    Interrupted(u32, i32),
    #[error(display = "process ({}) had a core dump", _0)]
    CoreDump(u32),
    #[error(display = "waitpid error: {}", _0)]
    WaitPid(&'static str),

    #[error(display = "early exit: pipeline failed")]
    EarlyExit,
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
pub unsafe fn stdin_of<T: AsRef<[u8]>>(input: T) -> Result<RawFd, io::Error> {
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

impl Input {
    pub fn get_infile(&mut self) -> Result<File, InputError> {
        match self {
            Input::File(ref filename) => match File::open(filename.as_str()) {
                Ok(file) => Ok(file),
                Err(why) => Err(InputError::File { file: filename.to_string(), why }),
            },
            Input::HereString(ref mut string) => {
                if !string.ends_with('\n') {
                    string.push('\n');
                }

                match unsafe { stdin_of(&string) } {
                    Ok(stdio) => Ok(unsafe { File::from_raw_fd(stdio) }),
                    Err(why) => Err(InputError::HereString { string: string.to_string(), why }),
                }
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
    for PipeItem { job, outputs, mut inputs } in pipeline.items.into_iter() {
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
        if let Some(Value::Function(function)) = self.variables.get_ref(name).cloned() {
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
            duplicate_streams().map_err(PipelineError::CreatePipeError)?;
        redirect_streams(&job.stdin, &job.stdout, &job.stderr);
        let code = match job.var {
            JobVariant::Builtin { ref main } => main(job.args(), self),
            JobVariant::Function => self.exec_function(job.command(), job.args()),
            _ => panic!("exec job should not be able to be called on Cat or Tee jobs"),
        };
        redirect_streams(&stdin_bk, &Some(stdout_bk), &Some(stderr_bk));
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
        } else if let Some(Value::Function(_)) = self.variables.get_ref(&job.args[0]) {
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
                    let _ = sys::tcsetpgrp(0, process::id());
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
                let (mut pgid, mut last_pid, mut current_pid) = (0, 0, 0);

                // Append jobs until all piped jobs are running
                for (mut child, ckind) in commands {
                    // Keep a reference to the FD to keep them open
                    let mut ext_stdio_pipes: Option<Vec<File>> = None;

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
                            .connect(tee_out, tee_err)?;
                    } else {
                        // Pipe the previous command's stdin to this commands stdout/stderr.
                        let (reader, writer) =
                            sys::pipe2(sys::O_CLOEXEC).map_err(PipelineError::CreatePipeError)?;
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

                    spawn_proc(
                        self,
                        parent,
                        kind,
                        true,
                        &mut last_pid,
                        &mut current_pid,
                        &mut pgid,
                    )?;

                    last_pid = current_pid;
                    parent = child;
                    kind = ckind;
                    if ckind == RedirectFrom::None {
                        break;
                    }
                }

                spawn_proc(self, parent, kind, false, &mut last_pid, &mut current_pid, &mut pgid)?;

                // Waits for all of the children of the assigned pgid to finish executing,
                // returning the exit status of the last process in the queue.
                // Watch the foreground group, dropping all commands that exit as they exit.
                let status = self.watch_foreground(pgid)?;
                if status == Status::TERMINATED {
                    sys::killpg(pgid, sys::SIGTERM).map_err(PipelineError::TerminateJobsError)?;
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
    block_child: bool,
    last_pid: &mut u32,
    current_pid: &mut u32,
    pgid: &mut u32,
) -> Result<(), PipelineError> {
    let RefinedJob { mut var, args, stdin, stdout, stderr } = cmd;
    match var {
        JobVariant::External => {
            let mut result = sys::fork_and_exec(
                &args[0],
                &args[1..],
                stdin.as_ref().map(AsRawFd::as_raw_fd),
                stdout.as_ref().map(AsRawFd::as_raw_fd),
                stderr.as_ref().map(AsRawFd::as_raw_fd),
                false,
                || prepare_child(block_child, *pgid),
            );

            match result {
                Ok(pid) => {
                    *last_pid = *current_pid;
                    *current_pid = pid;
                }
                Err(ref mut err) if err.kind() == io::ErrorKind::NotFound => {
                    shell.command_not_found(&args[0])
                }
                Err(cause) => return Err(PipelineError::CommandExecError(cause)),
            }
        }
        JobVariant::Builtin { main } => {
            fork_exec_internal(
                stdout,
                stderr,
                stdin,
                block_child,
                last_pid,
                current_pid,
                *pgid,
                |_, _, _| main(&args, shell),
            )?;
        }
        JobVariant::Function => {
            fork_exec_internal(
                stdout,
                stderr,
                stdin,
                block_child,
                last_pid,
                current_pid,
                *pgid,
                |_, _, _| shell.exec_function(&args[0], &args),
            )?;
        }
        JobVariant::Cat { ref mut sources } => {
            fork_exec_internal(
                stdout,
                None,
                stdin,
                block_child,
                last_pid,
                current_pid,
                *pgid,
                |_, _, mut stdin| shell.exec_multi_in(sources, &mut stdin),
            )?;
        }
        JobVariant::Tee { ref mut items } => {
            fork_exec_internal(
                stdout,
                stderr,
                stdin,
                block_child,
                last_pid,
                current_pid,
                *pgid,
                |_, _, _| shell.exec_multi_out(items, redirection),
            )?;
        }
    };
    set_process_group(pgid, *current_pid, !shell.opts.is_background_shell);
    resume_process(*last_pid);
    Ok(())
}

// TODO: Integrate this better within the RefinedJob type.
fn fork_exec_internal<F>(
    stdout: Option<File>,
    stderr: Option<File>,
    stdin: Option<File>,
    block_child: bool,
    last_pid: &mut u32,
    current_pid: &mut u32,
    pgid: u32,
    mut exec_action: F,
) -> Result<(), PipelineError>
where
    F: FnMut(Option<File>, Option<File>, Option<File>) -> Status,
{
    match unsafe { sys::fork() } {
        Ok(0) => {
            prepare_child(block_child, pgid);

            redirect_streams(&stdin, &stdout, &stderr);
            let exit_status = exec_action(stdout, stderr, stdin);
            exit(exit_status.as_os_code())
        }
        Ok(pid) => {
            *last_pid = *current_pid;
            *current_pid = pid;
            Ok(())
        }
        Err(cause) => Err(PipelineError::CreateForkError(cause)),
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

fn resume_process(pid: u32) {
    if pid != 0 {
        // Ensure that the process is stopped before continuing.
        if let Err(why) = wait_for_interrupt(pid) {
            eprintln!("ion: error waiting for sigstop: {}", why);
        }
        let _ = sys::kill(pid, sys::SIGCONT);
    }
}

fn set_process_group(pgid: &mut u32, pid: u32, set_foreground: bool) {
    if *pgid == 0 {
        *pgid = pid;
        if set_foreground {
            let _ = sys::tcsetpgrp(0, pid);
        }
    }
    let _ = sys::setpgid(pid, *pgid);
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
