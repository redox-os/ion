use std::io::{self, Error, Write};
use std::process::{Command, exit};
use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::fs::{File, OpenOptions};
use super::flags::*;
use super::fork::{fork_pipe, create_process_group};
use super::job_control::JobControl;
use super::{JobKind, Shell};
use super::job::RefinedJob;
use super::status::*;
use super::signals::{self, SignalHandler};
use parser::peg::{Pipeline, Input, RedirectFrom};
use sys;

/// Use dup2 to replace `old` with `new` using `old`s file descriptor ID
pub fn redir(old: RawFd, new: RawFd) {
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
fn check_if_background_job(pipeline: &Pipeline, print_comm: bool) -> Option<String> {
    if pipeline.jobs[pipeline.jobs.len()-1].kind == JobKind::Background {
        let command = pipeline.to_string();
        if print_comm { eprintln!("> {}", command); }
        Some(command)
    } else if print_comm {
        eprintln!("> {}", pipeline.to_string());
        None
    } else {
        None
    }
}

pub trait PipelineExecution {
    fn execute_pipeline(&mut self, pipeline: &mut Pipeline) -> i32;
}

impl<'a> PipelineExecution for Shell<'a> {
    fn execute_pipeline(&mut self, pipeline: &mut Pipeline) -> i32 {
        let background_string = check_if_background_job(&pipeline, self.flags & PRINT_COMMS != 0);

        let mut piped_commands: Vec<(RefinedJob, JobKind)> = {
            pipeline.jobs
                .drain(..)
                .map(|mut job| {
                    let refined = {
                        if self.builtins.contains_key::<str>(
                            job.command.as_ref()
                        ) {
                            RefinedJob::builtin(
                                job.command,
                                job.args.drain().collect()
                            )
                        } else {
                            let mut command = Command::new(job.command);
                            for arg in job.args.drain().skip(1) {
                                command.arg(arg);
                            }
                            RefinedJob::External(command)
                        }
                    };
                    (refined, job.kind)
                })
                .collect()
        };
        match pipeline.stdin {
            None => (),
            Some(Input::File(ref filename)) => {
                if let Some(command) = piped_commands.first_mut() {
                    match File::open(filename) {
                        Ok(file) => command.0.stdin(file.into_raw_fd()),
                        Err(e) => {
                            eprintln!("ion: failed to redirect '{}' into stdin: {}",
                                      filename, e)
                        }
                    }
                }
            },
            Some(Input::HereString(ref mut string)) => {
                if let Some(command) = piped_commands.first_mut() {
                    if !string.ends_with('\n') { string.push('\n'); }
                    match unsafe { stdin_of(&string) } {
                        Ok(stdio) => {
                            command.0.stdin(stdio);
                        },
                        Err(e) => {
                            eprintln!(
                                "ion: failed to redirect herestring '{}' into stdin: {}",
                                string,
                                e
                            );
                        }
                    }
                }
            }
        }

        if let Some(ref stdout) = pipeline.stdout {
            if let Some(mut command) = piped_commands.last_mut() {
                let file = if stdout.append {
                    OpenOptions::new().create(true).write(true).append(true).open(&stdout.file)
                } else {
                    File::create(&stdout.file)
                };
                match file {
                    Ok(f) => match stdout.from {
                        RedirectFrom::Both => {
                            match f.try_clone() {
                                Ok(f_copy) => {
                                    command.0.stdout(f.into_raw_fd());
                                    command.0.stderr(f_copy.into_raw_fd());
                                },
                                Err(e) => {
                                    eprintln!("ion: failed to redirect both stderr and stdout into file '{:?}': {}", f, e);
                                }
                            }
                        },
                        RedirectFrom::Stderr => command.0.stderr(f.into_raw_fd()),
                        RedirectFrom::Stdout => command.0.stdout(f.into_raw_fd()),
                    },
                    Err(err) => {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "ion: failed to redirect stdout into {}: {}", stdout.file, err);
                    }
                }
            }
        }

        self.foreground.clear();
        // If the given pipeline is a background task, fork the shell.
        if let Some(command_name) = background_string {
            fork_pipe(self, piped_commands, command_name)
        } else {
            // While active, the SIGTTOU signal will be ignored.
            let _sig_ignore = SignalHandler::new();
            // Execute each command in the pipeline, giving each command the foreground.
            let exit_status = pipe(self, piped_commands, true);
            // Set the shell as the foreground process again to regain the TTY.
            let _ = sys::tcsetpgrp(0, sys::getpid().unwrap());
            exit_status
        }
    }
}

/// This function will panic if called with an empty slice
pub fn pipe (
    shell: &mut Shell,
    commands: Vec<(RefinedJob, JobKind)>,
    foreground: bool
) -> i32 {
    let mut previous_status = SUCCESS;
    let mut previous_kind = JobKind::And;
    let mut commands = commands.into_iter();
    loop {
        if let Some((mut parent, mut kind)) = commands.next() {
            // When an `&&` or `||` operator is utilized, execute commands based on the previous status.
            match previous_kind {
                JobKind::And => if previous_status != SUCCESS {
                    if let JobKind::Or = kind { previous_kind = kind }
                    continue
                },
                JobKind::Or => if previous_status == SUCCESS {
                    if let JobKind::And = kind { previous_kind = kind }
                    continue
                },
                _ => ()
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
                                                if foreground {
                                                    let _ = sys::tcsetpgrp(0, pgid);
                                                }
                                            }
                                            shell.foreground.push(child.id());
                                            children.push(child.id());
                                        },
                                        Err(e) => {
                                            eprintln!("ion: failed to spawn `{}`: {}",
                                                      short, e);
                                            return NO_SUCH_COMMAND
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
                                            create_process_group(pgid);
                                            let args: Vec<&str> = args
                                                .iter()
                                                .map(|x| x as &str).collect();
                                            builtin(shell,
                                                    name,
                                                    &args,
                                                    *stdout,
                                                    *stderr,
                                                    *stdin);
                                        },
                                        Ok(pid) => {
                                            if pgid == 0 {
                                                pgid = pid;
                                                if foreground {
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
                            }
                        };
                    }

                    // Append other jobs until all piped jobs are running
                    while let Some((mut child, ckind)) = commands.next() {
                        match sys::pipe2(sys::O_CLOEXEC) {
                            Err(e) =>  {
                                eprintln!("ion: failed to create pipe: {:?}", e);
                            },
                            Ok((reader, writer)) => {
                                child.stdin(reader);
                                match mode {
                                    RedirectFrom::Stderr => {
                                        parent.stderr(writer);
                                    },
                                    RedirectFrom::Stdout => {
                                        parent.stdout(writer);
                                    },
                                    RedirectFrom::Both => {
                                        let temp = unsafe {
                                            File::from_raw_fd(writer)
                                        };
                                        match temp.try_clone() {
                                            Err(e) => {
                                                eprintln!("ion: failed to redirect stdout and stderr: {}", e);
                                            }
                                            Ok(duped) => {
                                                parent.stderr(temp.into_raw_fd());
                                                parent.stdout(duped.into_raw_fd());
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
                            break
                        }
                    }
                    previous_kind = kind;
                    previous_status = wait(shell, children, remember);
                    if previous_status == TERMINATED {
                        shell.foreground_send(sys::SIGTERM);
                        return previous_status;
                    }
                }
                _ => {
                    previous_status = execute(shell, &mut parent, foreground);
                    previous_kind = kind;
                }
            }
        } else {
            break
        }
    }
    previous_status
}

fn execute(shell: &mut Shell, job: &mut RefinedJob, foreground: bool) -> i32 {
    let short = job.short();
    let long = job.long();
    match *job {
        RefinedJob::External(ref mut command) => {
            match {
                command.before_exec(move || {
                    signals::unblock();
                    create_process_group(0);
                    Ok(())
                }).spawn()
            } {
                Ok(child) => {
                    if foreground {
                        let _ = sys::tcsetpgrp(0, child.id());
                    }
                    shell.watch_foreground(child.id(), child.id(), move || long, |_| ())
                },
                Err(e) => {
                    if e.kind() == io::ErrorKind::NotFound {
                        eprintln!("ion: command not found: {}", short)
                    } else {
                        eprintln!("ion: error spawning process: {}", e)
                    };
                    FAILURE
                }
            }
        }
        RefinedJob::Builtin { ref name, ref args, ref stdin, ref stdout, ref stderr } => {
            match unsafe { sys::fork() } {
                Ok(0) => {
                    signals::unblock();
                    create_process_group(0);
                    let args: Vec<&str> = args.iter().map(|x| x as &str).collect();
                    builtin(shell, name, &args, *stdout, *stderr, *stdin)
                },
                Ok(pid) => {
                    if foreground {
                        let _ = sys::tcsetpgrp(0, pid);
                    }
                    shell.watch_foreground(pid, pid, move || long, |_| ())
                },
                Err(e) => {
                    eprintln!("ion: fork error for '{}': {}", short, e);
                    FAILURE
                }
            }
        }
    }
}

/// Waits for all of the children within a pipe to finish exuecting, returning the
/// exit status of the last process in the queue.
fn wait (
    shell: &mut Shell,
    mut children: Vec<u32>,
    mut commands: Vec<RefinedJob>
) -> i32 {
    // TODO: Find a way to only do this when absolutely necessary.
    let as_string = commands.iter()
        .map(RefinedJob::long)
        .collect::<Vec<String>>()
        .join(" | ");

    // Each process in the pipe has the same PGID, which is the first process's PID.
    let pgid = children[0];

    // If the last process exits, we know that all processes should exit.
    let last_pid = children[children.len()-1];

    // Watch the foreground group, dropping all commands that exit as they exit.
    shell.watch_foreground(pgid, last_pid, move || as_string, move |pid| {
        if let Some(id) = children.iter().position(|&x| x as i32 == pid) {
            commands.remove(id);
            children.remove(id);
        }
    })
}


/// Execute a builtin in the current process. Note that this will exit
/// the current process with the return code of the builtin
/// # Args
/// * `shell`: A `Shell` that forwards relevant information to the builtin
/// * `name`: Name of the builtin to execute.
/// * `stdin`, `stdout`, `stderr`: File descriptors that will replace the
///    respective standard streams if they are not `None`
/// # Preconditions
/// * `shell.builtins.contains_key(name)`; otherwise this function will panic
fn builtin(
    shell: &mut Shell,
    name: &str,
    args: &[&str],
    stdout: Option<RawFd>,
    stderr: Option<RawFd>,
    stdin: Option<RawFd>,
) -> ! {
    /// Close a file descriptor by opening a `File` and letting it drop
    fn close(fd: Option<RawFd>) {
        if let Some(fd) = fd {
            if let Err(e) = sys::close(fd) {
                eprintln!("ion: failed to close file '{}': {}", fd, e);
            }
        }
    }
    if let Some(fd) = stdin {
        redir(fd, sys::STDIN_FILENO);
    }
    if let Some(fd) = stdout {
        redir(fd, sys::STDOUT_FILENO);
    }
    if let Some(fd) = stderr {
        redir(fd, sys::STDERR_FILENO);
    }
    // The precondition for this function asserts that there exists some `builtin`
    // in `shell` named `name`, so we unwrap here
    let builtin = shell.builtins.get(name).unwrap();
    let ret = (builtin.main)(args, shell);
    close(stderr);
    close(stdout);
    close(stdin);
    exit(ret)
}
