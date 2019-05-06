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
    streams::{duplicate_streams, redirect_streams},
};
use super::{
    flags::*,
    flow_control::{Function, FunctionError},
    fork_function::command_not_found,
    job::{Job, JobVariant, RefinedJob, TeeItem},
    signals::{self, SignalHandler},
    status::*,
    Shell,
};
use crate::{
    builtins::{self, BuiltinFunction},
    parser::pipelines::{Input, PipeItem, PipeType, Pipeline, RedirectFrom, Redirection},
    sys,
};
use itertools::Itertools;
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
) -> Result<(), ()> {
    // XXX: Possibly add an assertion here for correctness
    for output in outputs {
        match OpenOptions::new()
            .create(true)
            .write(true)
            .append(output.append)
            .open(output.file.as_str())
        {
            Ok(f) => match output.from {
                RedirectFrom::None => (),
                RedirectFrom::Stdout => stdout(job, f),
                RedirectFrom::Stderr => stderr(job, f),
                RedirectFrom::Both => match f.try_clone() {
                    Ok(f_copy) => {
                        stdout(job, f);
                        stderr(job, f_copy);
                    }
                    Err(e) => {
                        eprintln!(
                            "ion: failed to redirect both stdout and stderr to file '{:?}': {}",
                            f, e
                        );
                        return Err(());
                    }
                },
            },
            Err(e) => {
                eprintln!("ion: failed to redirect output into {}: {}", output.file, e);
                return Err(());
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
) -> Result<impl IntoIterator<Item = (RefinedJob<'b>, RedirectFrom)>, ()> {
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
            stdin.as_ref().map(File::as_raw_fd),
            stdout.as_ref().map(File::as_raw_fd),
            stderr.as_ref().map(File::as_raw_fd),
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
                if let Err(_) = command_not_found(self, &name) {
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

    /// For tee jobs
    fn exec_multi_out(
        &mut self,
        items: &mut (Option<TeeItem>, Option<TeeItem>),
        redirection: RedirectFrom,
    ) -> i32 {
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
            eprintln!("ion: error in multiple output redirection process: {:?}", e);
            FAILURE
        } else {
            SUCCESS
        }
    }

    /// For cat jobs
    fn exec_multi_in(&mut self, sources: &mut [File], stdin: &mut Option<File>) -> i32 {
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

    fn exec_function<S: AsRef<str>>(&mut self, name: &str, args: &[S]) -> i32 {
        match self.variables.get::<Function>(name).unwrap().execute(self, args) {
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

    /// Execute a builtin in the current process.
    /// # Args
    /// * `shell`: A `Shell` that forwards relevant information to the builtin
    /// * `name`: Name of the builtin to execute.
    /// * `stdin`, `stdout`, `stderr`: File descriptors that will replace the respective standard
    ///   streams if they are not `None`
    fn exec_builtin<'a>(&mut self, main: BuiltinFunction<'a>, args: &[small::String]) -> i32 {
        main(args, self)
    }

    /// Executes a `RefinedJob` that was created in the `generate_commands` method.
    ///
    /// The aforementioned `RefinedJob` may be either a builtin or external command.
    /// The purpose of this function is therefore to execute both types accordingly.
    fn exec_job(&mut self, job: &RefinedJob<'b>, _foreground: bool) -> i32 {
        // Duplicate file descriptors, execute command, and redirect back.
        if let Ok((stdin_bk, stdout_bk, stderr_bk)) = duplicate_streams() {
            redirect_streams(&job.stdin, &job.stdout, &job.stderr);
            let code = match job.var {
                JobVariant::External { ref args } => {
                    self.exec_external(&args[0], &args[1..], &job.stdin, &job.stdout, &job.stderr)
                }
                JobVariant::Builtin { ref main, ref args } => self.exec_builtin(main, &**args),
                JobVariant::Function { ref args } => self.exec_function(&args[0], args),
                _ => panic!("exec job should not be able to be called on Cat or Tee jobs"),
            };
            redirect_streams(&stdin_bk, &Some(stdout_bk), &Some(stderr_bk));
            code
        } else {
            eprintln!(
                "ion: failed to `dup` STDOUT, STDIN, or STDERR: not running '{}'",
                job.long()
            );

            COULD_NOT_EXEC
        }
    }

    /// Waits for all of the children of the assigned pgid to finish executing, returning the
    /// exit status of the last process in the queue.
    #[inline]
    fn wait(&mut self, pgid: u32, commands: SmallVec<[RefinedJob<'b>; 16]>) -> i32 {
        // Watch the foreground group, dropping all commands that exit as they exit.
        self.watch_foreground(-(pgid as i32), commands.iter().map(RefinedJob::long).join(" | "))
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
        } else if self.variables.get::<Function>(&job.args[0]).is_some() {
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
    pub fn execute_pipeline(&mut self, pipeline: Pipeline<'b>) -> i32 {
        // Don't execute commands when the `-n` flag is passed.
        if self.flags & NO_EXEC != 0 {
            return SUCCESS;
        }

        // A string representing the command is stored here.
        let command = pipeline.to_string();
        if self.flags & PRINT_COMMS != 0 {
            eprintln!("> {}", command);
        }

        // If the given pipeline is a background task, fork the shell.
        match pipeline.pipe {
            PipeType::Disown => fork_pipe(self, pipeline, command, ProcessState::Empty),
            PipeType::Background => fork_pipe(self, pipeline, command, ProcessState::Running),
            PipeType::Normal => {
                // While active, the SIGTTOU signal will be ignored.
                let _sig_ignore = SignalHandler::new();
                let foreground = !self.is_background_shell;
                // Execute each command in the pipeline, giving each command the foreground.
                let exit_status = pipe(self, pipeline, foreground);
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
pub(crate) fn pipe<'a>(shell: &mut Shell<'a>, pipeline: Pipeline<'a>, foreground: bool) -> i32 {
    let mut commands = match prepare(shell, pipeline) {
        Ok(c) => c.into_iter().peekable(),
        Err(_) => return COULD_NOT_EXEC,
    };

    if let Some((mut parent, mut kind)) = commands.next() {
        if kind != RedirectFrom::None {
            let (mut pgid, mut last_pid, mut current_pid) = (0, 0, 0);

            // Append jobs until all piped jobs are running
            while let Some((mut child, ckind)) = commands.next() {
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
                        .connect(tee_out, tee_err);
                } else {
                    // Pipe the previous command's stdin to this commands stdout/stderr.
                    match sys::pipe2(sys::O_CLOEXEC) {
                        Err(e) => pipe_fail(e),
                        Ok((reader, writer)) => {
                            if is_external {
                                append_external_stdio_pipe(&mut ext_stdio_pipes, writer);
                            }
                            child.stdin(unsafe { File::from_raw_fd(reader) });
                            let writer = unsafe { File::from_raw_fd(writer) };
                            match kind {
                                RedirectFrom::None => (),
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

                spawn_proc(shell, parent, kind, true, &mut last_pid, &mut current_pid, pgid);
                if set_process_group(&mut pgid, current_pid) && foreground && !shell.is_library {
                    let _ = sys::tcsetpgrp(0, pgid);
                }
                resume_prior_process(&mut last_pid, current_pid);

                parent = child;
                if ckind != RedirectFrom::None {
                    kind = ckind;
                } else {
                    kind = ckind;
                    break;
                }
            }

            spawn_proc(shell, parent, kind, false, &mut last_pid, &mut current_pid, pgid);
            if set_process_group(&mut pgid, current_pid) && foreground && !shell.is_library {
                let _ = sys::tcsetpgrp(0, pgid);
            }
            resume_prior_process(&mut last_pid, current_pid);

            let status = shell.wait(pgid, SmallVec::new());
            if status == TERMINATED {
                if let Err(why) = sys::killpg(pgid, sys::SIGTERM) {
                    eprintln!("ion: failed to terminate foreground jobs: {}", why);
                }
            } else {
                let _ = io::stdout().flush();
                let _ = io::stderr().flush();
            }
            status
        } else {
            let status = shell.exec_job(&parent, foreground);

            let _ = io::stdout().flush();
            let _ = io::stderr().flush();

            status
        }
    } else {
        SUCCESS
    }
}

fn spawn_proc(
    shell: &mut Shell,
    mut cmd: RefinedJob,
    redirection: RedirectFrom,
    block_child: bool,
    last_pid: &mut u32,
    current_pid: &mut u32,
    pgid: u32,
) {
    let stdin = cmd.stdin;
    let stdout = cmd.stdout;
    let stderr = cmd.stderr;
    match cmd.var {
        JobVariant::External { ref args } => {
            let name = &args[0];
            let args: Vec<&str> = args.iter().skip(1).map(|x| x as &str).collect();
            let mut result = sys::fork_and_exec(
                name,
                &args,
                stdin.as_ref().map(AsRawFd::as_raw_fd),
                stdout.as_ref().map(AsRawFd::as_raw_fd),
                stderr.as_ref().map(AsRawFd::as_raw_fd),
                false,
                || prepare_child(block_child, pgid),
            );

            match result {
                Ok(pid) => {
                    *last_pid = *current_pid;
                    *current_pid = pid;
                }
                Err(ref mut err) if err.kind() == io::ErrorKind::NotFound => {
                    if let Err(_) = command_not_found(shell, &name) {
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
                |_, _, _| shell.exec_builtin(main, args),
            );
        }
        JobVariant::Function { ref mut args } => {
            fork_exec_internal(
                stdout,
                stderr,
                stdin,
                block_child,
                last_pid,
                current_pid,
                pgid,
                |_, _, _| shell.exec_function(&args[0], &args),
            );
        }
        JobVariant::Cat { ref mut sources } => {
            fork_exec_internal(
                stdout,
                None,
                stdin,
                block_child,
                last_pid,
                current_pid,
                pgid,
                |_, _, mut stdin| shell.exec_multi_in(sources, &mut stdin),
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
                |_, _, _| shell.exec_multi_out(items, redirection),
            );
        }
    }
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
) where
    F: FnMut(Option<File>, Option<File>, Option<File>) -> i32,
{
    match unsafe { sys::fork() } {
        Ok(0) => {
            prepare_child(block_child, pgid);

            redirect_streams(&stdin, &stdout, &stderr);
            let exit_status = exec_action(stdout, stderr, stdin);
            exit(exit_status)
        }
        Ok(pid) => {
            *last_pid = *current_pid;
            *current_pid = pid;
        }
        Err(e) => pipe_fail(e),
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

#[inline]
pub fn pipe_fail(why: io::Error) {
    eprintln!("ion: failed to create pipe: {:?}", why);
}

pub fn append_external_stdio_pipe(pipes: &mut Option<Vec<File>>, file: RawFd) {
    pipes.get_or_insert_with(|| Vec::with_capacity(4)).push(unsafe { File::from_raw_fd(file) });
}
